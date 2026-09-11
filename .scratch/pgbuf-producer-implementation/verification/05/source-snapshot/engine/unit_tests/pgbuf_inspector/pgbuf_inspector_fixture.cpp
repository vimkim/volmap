/*
 * Copyright 2008 Search Solution Corporation
 * Copyright 2016 CUBRID Corporation
 *
 *  Licensed under the Apache License, Version 2.0 (the "License");
 *  you may not use this file except in compliance with the License.
 *  You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 *  Unless required by applicable law or agreed to in writing, software
 *  distributed under the License is distributed on an "AS IS" BASIS,
 *  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 *  See the License for the specific language governing permissions and
 *  limitations under the License.
 *
 */

#include "config.h"
#include "boot_sr.h"
#include "critical_section.h"
#include "error_manager.h"
#include "file_manager.h"
#include "log_impl.h"
#include "log_manager.h"
#include "message_catalog.h"
#include "page_buffer.h"
#include "system_parameter.h"
#include "thread_manager.hpp"
#include "tz_support.h"

#include <cstdio>
#include <cstring>
#include <cerrno>
#include <chrono>
#include <poll.h>
#include <unistd.h>

// XXX: SHOULD BE THE LAST INCLUDE HEADER
#include "memory_wrapper.hpp"

namespace
{
  THREAD_ENTRY *fixture_thread = nullptr;

  bool boot (const char *database)
  {
    if (er_init (nullptr, ER_NEVER_EXIT) != NO_ERROR)
      {
	return false;
      }
    cubthread::initialize (fixture_thread);
    if (!fixture_thread || msgcat_init () != NO_ERROR || tz_load () != NO_ERROR)
      {
	return false;
      }
    if (sysprm_load_and_init (database, nullptr, SYSPRM_LOAD_ALL) != NO_ERROR)
      {
	return false;
      }
    if (sync_initialize_sync_stats () != NO_ERROR || csect_initialize_static_critical_sections () != NO_ERROR)
      {
	return false;
      }
    CHECK_ARGS checks = {true, true};
    if (boot_restart_server (fixture_thread, false, database, false, &checks, nullptr, true) != NO_ERROR)
      {
	return false;
      }
    TRAN_STATE state;
    return logtb_assign_tran_index (fixture_thread, NULL_TRANID, TRAN_ACTIVE, nullptr, &state,
				    TRAN_LOCK_INFINITE_WAIT, TRAN_READ_COMMITTED) != NULL_TRAN_INDEX;
  }

  void reply (const char *state, const VPID &vpid)
  {
    std::printf ("PGFIXTURE {\"state\":\"%s\",\"volid\":%d,\"pageid\":%d}\n", state, vpid.volid, vpid.pageid);
    std::fflush (stdout);
  }

  bool owns_write_fix (PAGE_PTR page, const VPID &vpid)
  {
    // Allocation gave this single-threaded owner a WRITE fix, which has not
    // been released. These native checks also exist in release builds.
    return page && pgbuf_get_fix_count (page) == 1
	   && pgbuf_get_latch_mode (page) == PGBUF_LATCH_WRITE
	   && VPID_EQ (pgbuf_get_vpid_ptr (page), &vpid);
  }

  bool read_command (char (&command)[32])
  {
    // Bound the whole command, including a controller that sends only a prefix.
    const auto deadline = std::chrono::steady_clock::now () + std::chrono::seconds (10);
    for (std::size_t length = 0; length + 1 < sizeof (command);)
      {
	auto remaining = std::chrono::duration_cast<std::chrono::milliseconds> (
				 deadline - std::chrono::steady_clock::now ()).count ();
	if (remaining <= 0)
	  {
	    return false;
	  }
	pollfd input {STDIN_FILENO, POLLIN, 0};
	int ready = poll (&input, 1, static_cast<int> (remaining));
	if (ready < 0 && errno == EINTR)
	  {
	    continue;
	  }
	if (ready <= 0)
	  {
	    return false;
	  }
	ssize_t count = read (STDIN_FILENO, &command[length], 1);
	if (count < 0 && errno == EINTR)
	  {
	    continue;
	  }
	if (count != 1)
	  {
	    return false;
	  }
	if (command[length++] == '\n')
	  {
	    command[length] = '\0';
	    return true;
	  }
      }
    return false;
  }
}

// Test-only executable: normal engine boot and producer daemon, private stdin
// synchronization. It is neither installed nor exposed as a production command.
int main (int argc, char **argv)
{
  if (argc != 2 || !boot (argv[1]))
    {
      std::fprintf (stderr, "fixture boot failed: %d\n", er_errid ());
      return 1;
    }
  VFID file;
  VFID_SET_NULL (&file);
  VPID target;
  VPID_SET_NULL (&target);
  PAGE_PTR held = nullptr;
  PAGE_TYPE kind = PAGE_QRESULT;
  int result = 1;
  if (file_create_temp (fixture_thread, 1, &file) != NO_ERROR
      || file_alloc (fixture_thread, &file, file_init_temp_page_type, &kind, &target, &held) != NO_ERROR
      || !held || !pgbuf_flush_with_wal (fixture_thread, held) || !owns_write_fix (held, target))
    {
      std::fprintf (stderr, "fixture allocation/clean flush failed: %d\n", er_errid ());
    }
  else
    {
      reply ("clean-held", target);
      for (;;)
	{
	  char command[32];
	  if (!read_command (command))
	    {
	      break;
	    }
	  if (std::strcmp (command, "dirty\n") == 0 && held)
	    {
	      const char canary[] = "pgbuf-fixture-private-page-bytes";
	      std::memcpy (held, canary, sizeof (canary));
	      pgbuf_set_dirty (fixture_thread, held, DONT_FREE);
	      if (!owns_write_fix (held, target))
		{
		  break;
		}
	      reply ("dirty-held", target);
	    }
	  else if (std::strcmp (command, "held\n") == 0 && held)
	    {
	      if (!owns_write_fix (held, target))
		{
		  break;
		}
	      reply ("held", target);
	    }
	  else if (std::strcmp (command, "populate\n") == 0)
	    {
	      bool populated = true;
	      for (int i = 0; i < 512; ++i)
		{
		  VPID page_id;
		  PAGE_PTR page = nullptr;
		  if (file_alloc (fixture_thread, &file, file_init_temp_page_type, &kind, &page_id, &page) != NO_ERROR)
		    {
		      populated = false;
		      break;
		    }
		  pgbuf_unfix (fixture_thread, page);
		}
	      if (!populated)
		{
		  break;
		}
	      reply ("populated", target);
	    }
	  else if (std::strcmp (command, "stop\n") == 0)
	    {
	      result = 0;
	      break;
	    }
	  else
	    {
	      break;
	    }
	}
    }
  if (held)
    {
      pgbuf_unfix (fixture_thread, held);
    }
  if (!VFID_ISNULL (&file) && file_temp_retire (fixture_thread, &file) != NO_ERROR)
    {
      result = 1;
    }
  int transaction = LOG_FIND_THREAD_TRAN_INDEX (fixture_thread);
  if (transaction > LOG_SYSTEM_TRAN_INDEX)
    {
      log_abort (fixture_thread, transaction);
      logtb_release_tran_index (fixture_thread, transaction);
    }
  xboot_shutdown_server (fixture_thread, ER_THREAD_FINAL);
  if (result != 0)
    {
      std::fprintf (stderr, "fixture precondition/synchronization failed: %d\n", er_errid ());
    }
  return result;
}
