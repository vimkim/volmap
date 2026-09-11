from pathlib import Path
import hashlib, json, re, shutil, subprocess
base=Path(__file__).resolve().parent
engine=Path('/home/vimkim/gh/cb/CBRD-27398-pgbuf-inspector-contract')
tests=Path('/home/vimkim/gh/tc/CBRD-27398-pgbuf-inspector-fixtures')
def git(repo,*args): return subprocess.check_output(['git','-C',str(repo),*args],text=True).strip()
def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def artifact(label,log):
    match=re.search(r'Evidence directory: (.+)',log.read_text())
    assert match, log
    root=Path(match.group(1).strip())
    dest=base/label
    dest.mkdir(exist_ok=True)
    for p in root.iterdir():
        if p.is_file() and p.suffix in ('.json','.jsonl','.log','.conf','.out'):
            shutil.copy2(p,dest/p.name)
    return {'original':str(root),'retained':str(dest),'sha256':{p.name:sha(p) for p in dest.iterdir() if p.is_file()}}
results={}
artifacts={}
for mode in ('debug','release'):
    full=(base/(mode+'-full-final.log')).read_text()
    assert '100% tests passed, 0 tests failed out of 28' in full
    assert re.search(r'All tests passed \([0-9]+ assertions in 61 test cases\)',full)
    case=base/('ctp-'+mode+'-final-case')
    ctp=(base/('ctp-'+mode+'-final.log')).read_text()
    for key,value in [('Total Execution Case',1),('Total Success Case',1),('Total Fail Case',0),('Total Skip Case',0)]:
        assert re.search(key+':'+str(value)+r'\s',ctp)
    assert re.search(r'\[TESTCASE\].*cbrd_27398.sh.*\[OK\]',ctp)
    controlled=(case/'controlled.log').read_text()
    passes=[line for line in controlled.splitlines() if line.startswith('PASS ')]
    assert len(passes)==6, passes
    attachment=(case/'attachment.log').read_text()
    for marker in ['PASS default-off','PASS real resident scan','PASS restart changes incarnation','PASS activation failure']:
        assert marker in attachment
    artifacts[mode+'-native-final']=artifact(mode+'-native-final',case/'controlled.log')
    artifacts[mode+'-attachment-final']=artifact(mode+'-attachment-final',case/'attachment.log')
    native=Path(artifacts[mode+'-native-final']['retained'])
    sync=json.loads((native/'synchronization.json').read_text())
    target=(sync[0]['ack']['volid'],sync[0]['ack']['pageid'])
    assert all((x['ack']['volid'],x['ack']['pageid'])==target for x in sync)
    assert all(x['elapsed'] < (25 if x['command']=='evict' else 8) for x in sync)
    assert [x['command'] for x in sync].count('absent')==4
    assert 'cache-only target miss before workload cleanup' in (native/'native.log').read_text()
    for phase in ('clean','dirty','evicted-complete','evicted-partial'):
        frames=[json.loads(x) for x in (native/(phase+'.jsonl')).read_text().splitlines()]
        footer=frames[-1]
        assert footer['type']=='scan_footer'
        assert footer['truncated'] is (phase=='evicted-partial')
        pages=[x for x in frames if x['type']=='page']
        assert footer['record_count']==len(pages)
        matches=[p for p in pages if (p['volid'],p['pageid'])==target]
        if phase in ('clean','dirty'):
            assert len(matches)==1 and matches[0]['dirty'] is (phase=='dirty')
            assert matches[0]['fix_count']==1 and matches[0]['latch_mode']=='write'
        else: assert not matches
    for phase,state in [('evicted-complete','observed-nonresident'),('evicted-partial','unknown')]:
        assert json.loads((native/(phase+'-conclusion.json')).read_text())['state']==state
    results[mode]={'ctest_entries':28,'ctest_failed':0,'inspector_cases':61,'native_cases':passes,'external_ctp':{'executed':1,'success':1,'failed':0,'skipped':0},'known_vpid':target}
    install=Path('/home/vimkim/.cub/install/CBRD-27398-pgbuf-inspector-contract')/(mode+'_gcc')
    build=engine/('build_preset_'+mode+'_gcc')
    binaries=[install/'bin/cub_server',install/'lib/libcubrid.so',build/'bin/pgbuf_inspector_fixture']
    native_text=(native/'native.log').read_text()
    results[mode]['replacement_allocations']=int(re.search(r'native replacement: allocated=([0-9]+)',native_text).group(1))
    results[mode]['cleanup_retry_pages']=native_text.count('cleanup retry:')
    results[mode]['build_type']=re.search(r'^CMAKE_BUILD_TYPE:STRING=(.+)$',(build/'CMakeCache.txt').read_text(),re.M).group(1)
    results[mode]['binaries']={str(p):sha(p) for p in binaries}
    identity=(case/'engine-identity.log').read_text()
    results[mode]['reported_version']=next(x for x in identity.splitlines() if x.startswith('CUBRID 11.'))
    assert git(engine,'rev-parse','HEAD') in identity
    assert git(tests,'rev-parse','HEAD') in identity
    for digest in results[mode]['binaries'].values(): assert digest in identity
engine_tree=git(engine,'rev-parse','HEAD^{tree}')
test_tree=git(tests,'rev-parse','HEAD^{tree}')
assert engine_tree=='890d5c4c6e505ef93f55abb86d66bfe72e136420'
assert test_tree=='5b1f7792534f65b85b09fd4de21afe9a9badd5ea'
manifest={'ticket':'05','status':'complete','engine_commit':git(engine,'rev-parse','HEAD'),'engine_tree':engine_tree,'engine_base':'e5f3cdf86908b8d23d4e09d28d22265bf240f7ad','testcase_commit':git(tests,'rev-parse','HEAD'),'testcase_tree':test_tree,'testcase_base':'a8d61f27443e3c8b56bcaf94b23a8b88dff9069a','testcase_worktree':str(tests),'testcase_path':'shell/_06_issues/_26_2h/cbrd_27398/cases/cbrd_27398.sh','results':results,'artifacts':artifacts,'review':{'path':'review.md','standards_findings':0,'spec_findings':0},'commands':{'native':'python3 unit_tests/pgbuf_inspector/controlled_observation.py --fixture BUILD/bin/pgbuf_inspector_fixture','ctest':'ctest --test-dir BUILD --output-on-failure','external':'$CTP_HOME/bin/ctp.sh shell -c RUN_SPECIFIC_CONF','local_build':'direnv exec . env CUBRID_TMP=/tmp/pgbuf05-MODE-suite just build-test','local_ctp':'direnv exec . timeout -s KILL 360 unshare -r --mount-proc -i -p -f -n --kill-child=SIGKILL -- bash verification/05/ctp-MODE_gcc/inside.sh'},'bounds':{'ack_seconds':8,'eviction_ack_seconds':25,'command_seconds':10,'replacement_seconds':15,'replacement_max_pages':32768,'cleanup_seconds':5},'failed_attempts':['release-full-before-cleanup-fix.log: successful invalidation left protected workload pages cached; cleanup now retries within the same total5s bound and requires native cache miss','debug-controlled-first.log: direct temporary-file destruction caused shutdown assertion; use file_temp_retire','release-bootstrap.log: Debug-only holder API replaced with public native fix-count/latch/VPID checks','debug-focused.log and debug-deadline-recheck.log: fixed-size drain failed to free a kernel send allocation; corrected real-progress drain passed both modes','debug-eviction-first.log: replacement succeeded but full buffer pool made scan partial; remove only non-target workload after independently proving target eviction'],'limitations':['No consumer implementation or integration is claimed; ticket06 owns that gate','No native replacement-policy telemetry is added; proof is actual capacity workload followed by cache-only native miss','Test-only owner boots production engine/daemon; unmodified installed server is independently exercised','Existing dirty CCI submodule preserved; production source unchanged since engine_base','Local CTP uses isolated user/PID/mount/IPC/network namespaces and copied install/configuration; broad suite service cleanup stays inside that namespace','Binary embedded version can name the build-time base; exact reviewed source trees and binary hashes identify executed code']}
(base/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print('Verified final ticket05 manifest:',base/'manifest.json')
