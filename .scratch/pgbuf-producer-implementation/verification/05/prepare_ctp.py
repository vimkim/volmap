"""Prepare disposable CTP runtime; host HOME value is never reassigned."""
from pathlib import Path
import os, shutil, socket, subprocess, sys
mode=sys.argv[1]
assert mode in ('debug_gcc','release_gcc')
base=Path(__file__).resolve().parent
runtime=base/('ctp-'+mode)
runtime.mkdir(exist_ok=False)
original=Path('/home/vimkim')
install=original/'.cub/install/CBRD-27398-pgbuf-inspector-contract'/mode
for source,target in [(install,runtime/'install'),(original/'CTP',runtime/'CTP')]:
    subprocess.run(['cp','-a','--reflink=auto',str(source),str(target)],check=True)
for name in ('home','original-home','databases','.CUBRID_SHELL_FM/conf','.CUBRID_SHELL_FM/databases'):
    (runtime/name).mkdir(parents=True,exist_ok=True)
for entry in original.iterdir():
    if entry.name not in ('CTP','CUBRID','.CUBRID_SHELL_FM','.bash_profile','.bashrc','.profile'):
        (runtime/'home'/entry.name).symlink_to(Path('/mnt/original-home')/entry.name)
for name,target in [('CTP','/mnt/CTP'),('CUBRID','/mnt/install'),('.CUBRID_SHELL_FM','/mnt/.CUBRID_SHELL_FM')]:
    (runtime/'home'/name).symlink_to(target)
for name in ('.bash_profile','.bashrc','.profile'):
    (runtime/'home'/name).write_text('# Private CTP namespace: retain the explicit caller environment.\n')
for entry in (runtime/'install/conf').iterdir():
    target=runtime/'.CUBRID_SHELL_FM/conf'/entry.name
    if entry.is_dir(): shutil.copytree(entry,target)
    else: shutil.copy2(entry,target)
(runtime/'databases/databases.txt').touch()
(runtime/'.CUBRID_SHELL_FM/databases/databases.txt').touch()
(runtime/'hosts').write_text('127.0.0.1 localhost '+socket.gethostname()+'\n::1 localhost\n')
conf=runtime/'CTP/conf/shell_ci.conf'
lines=[]
for line in conf.read_text().splitlines():
    key=line.split('=',1)[0].strip()
    if key=='testcase_update_yn': line='testcase_update_yn=false'
    if key=='testcase_retry_num': line='testcase_retry_num=0'
    if key in ('mail_notice_to','mail_notice_cc','mail_notice_bcc'): line=key+'='
    lines.append(line)
conf.write_text('\n'.join(lines)+'\n')
entry=runtime/'inside.sh'
entry.write_text('''#!/bin/bash
set -euo pipefail
mount --make-rprivate /
mount --bind "'''+str(runtime)+'''" /mnt
mount --bind /home/vimkim /mnt/original-home
mount --bind /mnt/home /home/vimkim
mount --bind /mnt/hosts /etc/hosts
mount -t tmpfs -o size=512m tmpfs /tmp
ip link set lo up
mkdir /tmp/pgbuf-ctp
export CUBRID=/mnt/install CUBRID_DATABASES=/mnt/databases CUBRID_TMP=/tmp/pgbuf-ctp
export PATH=/mnt/install/bin:$PATH LD_LIBRARY_PATH=/mnt/install/lib:/mnt/install/cci/lib
export CTP_HOME=/mnt/CTP TMPDIR=/home/vimkim/temp
export PGBUF_INSPECTOR_TEST_DIR=/home/vimkim/gh/cb/CBRD-27398-pgbuf-inspector-contract/unit_tests/pgbuf_inspector
export PGBUF_INSPECTOR_FIXTURE=/home/vimkim/gh/cb/CBRD-27398-pgbuf-inspector-contract/build_preset_'''+mode+'''/bin/pgbuf_inspector_fixture
printf 'namespace pid=%s net=%s ipc=%s\\n' "$(readlink /proc/self/ns/pid)" "$(readlink /proc/self/ns/net)" "$(readlink /proc/self/ns/ipc)"
set +e
cubrid-shell-debug.sh /home/vimkim/gh/tc/CBRD-27398-pgbuf-inspector-fixtures/shell/_06_issues/_26_2h/cbrd_27398
result=$?
mkdir -p /mnt/evidence
cp -a /tmp/shell_single*.log /tmp/shell_single*.conf /mnt/evidence/ 2>/dev/null
exit "$result"
''')
print(runtime)
