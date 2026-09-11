from pathlib import Path
import hashlib, json, re, shutil, subprocess
base=Path(__file__).resolve().parent
engine=Path('/home/vimkim/gh/cb/CBRD-27398-pgbuf-inspector-contract')
tests=Path('/home/vimkim/gh/tc/CBRD-27398-pgbuf-inspector-fixtures')
files=['unit_tests/pgbuf_inspector/test_pgbuf_inspector_socket.cpp','unit_tests/pgbuf_inspector/CMakeLists.txt','unit_tests/pgbuf_inspector/pgbuf_inspector_fixture.cpp','unit_tests/pgbuf_inspector/controlled_observation.py','docs/pgbuf-inspector/controlled-observation.md']
testfiles=['shell/_06_issues/_26_2h/cbrd_27398/cases/cbrd_27398.sh','shell/_06_issues/_26_2h/cbrd_27398/README.md']
def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()
sources={}
for repo,names,prefix in [(engine,files,'engine'),(tests,testfiles,'testcases')]:
    for name in names:
        dest=base/'source-snapshot'/prefix/name
        dest.parent.mkdir(parents=True,exist_ok=True)
        shutil.copy2(repo/name,dest)
        sources[prefix+'/'+name]=sha(dest)
    (base/(prefix+'-tracked.diff')).write_bytes(subprocess.check_output(['git','-C',str(repo),'diff','--binary','--ignore-submodules']))
artifacts={}
for label,log in [('debug-ctp-native',base/'ctp-debug-first-case/controlled.log'),('debug-ctp-attachment',base/'ctp-debug-first-case/attachment.log'),('release-ctp-native',base/'ctp-release-first-case/controlled.log'),('release-ctp-attachment',base/'ctp-release-first-case/attachment.log'),('release-native',base/'release-controlled-first.log'),('debug-current-native',engine/'build_preset_debug_gcc/Testing/Temporary/LastTest.log')]:
    match=re.search(r'Evidence directory: (.+)',log.read_text())
    if not match: continue
    root=Path(match.group(1).strip())
    dest=base/label
    dest.mkdir(exist_ok=True)
    for p in root.iterdir():
        if p.is_file() and p.suffix in ('.json','.jsonl','.log','.conf','.out'):
            shutil.copy2(p,dest/p.name)
    artifacts[label]={'original':str(root),'retained':str(dest),'files':{p.name:sha(p) for p in dest.iterdir() if p.is_file()}}
provenance={}
for mode in ('debug_gcc','release_gcc'):
    install=Path('/home/vimkim/.cub/install/CBRD-27398-pgbuf-inspector-contract')/mode
    paths=[install/'bin/cub_server',install/'lib/libcubrid.so',engine/('build_preset_'+mode)/'bin/pgbuf_inspector_fixture']
    cache=(engine/('build_preset_'+mode)/'CMakeCache.txt').read_text()
    provenance[mode]={'build_type':re.search(r'^CMAKE_BUILD_TYPE:STRING=(.+)$',cache,re.M).group(1),'sha256':{str(p):sha(p) for p in paths}}
manifest={
 'ticket':'05','status':'incomplete: eviction method clarification pending',
 'engine_base':subprocess.check_output(['git','-C',str(engine),'rev-parse','HEAD'],text=True).strip(),
 'testcase_base':subprocess.check_output(['git','-C',str(tests),'rev-parse','HEAD'],text=True).strip(),
 'testcase_worktree':str(tests),'testcase_branch':'CBRD-27398-pgbuf-inspector-fixtures',
 'source_snapshot_sha256':sources,'provenance':provenance,'artifacts':artifacts,
 'checks':{'native':['clean-held','dirty-held','partial-held','shutdown'],'release_partial_command_timeout':json.loads((base/'release-command-timeout.json').read_text()),'external_ctp':{'debug':{'executed':1,'success':1,'failed':0,'skipped':0},'release':{'executed':1,'success':1,'failed':0,'skipped':0}},'focused_ctest':{'debug':{'executed':2,'failed':0,'log':'debug-focused-corrected.log'},'release':{'executed':2,'failed':0}},'unmodified_release_attachment':'passed in external CTP companion'},
 'pending':['Eviction choice: explicit native invalidation or actual LRU replacement','Independent absence and no-refix proof across complete scan','Complete omission versus partial unknown assertions','Final debug/release full suites','Two-axis code review','Engine and external testcase commits/exact final revisions'],
 'failed_attempts':['debug-focused.log and debug-deadline-recheck.log: fixed-size draining did not free a kernel send allocation; corrected test to drain minimally until actual write progress; debug-focused-corrected.log passed','debug-controlled-first.log: wrong temporary-file destruction caused shutdown assertion; repaired with file_temp_retire','release-bootstrap.log: debug-only holder API unavailable in release; replaced with public fix-count/latch/VPID checks'],
 'limitations':['Debug first CTP run predates bounded command reader and release-compatible native checks; debug-focused.log verifies current source separately','No completed ticket05 gate is claimed; source remains uncommitted','Existing dirty cubrid-cci submodule preserved'],
 'ctp_isolation':'User/PID/mount/IPC/network namespaces; private CTP/install/home mount view; HOME environment unchanged; broad suite cleanup confined to namespace',
 'commands':['python3 unit_tests/pgbuf_inspector/controlled_observation.py --fixture BUILD/bin/pgbuf_inspector_fixture','ctest --test-dir BUILD -R ^test_pgbuf_inspector --output-on-failure','$CTP_HOME/bin/ctp.sh shell -c RUN_SPECIFIC_CONF (standard entrypoint; personal helper and namespace invocation retained in inside.sh / CTP transcripts)']
}
(base/'progress.json').write_text(json.dumps(manifest,indent=2)+'\n')
