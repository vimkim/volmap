from pathlib import Path
import json, os, subprocess, tempfile, time
base=Path(__file__).resolve().parent
root=Path(tempfile.mkdtemp(prefix='pgbuf-timeout-',dir='/home/vimkim/temp'))
install=Path('/home/vimkim/.cub/install/CBRD-27398-pgbuf-inspector-contract/release_gcc')
fixture=Path('/home/vimkim/gh/cb/CBRD-27398-pgbuf-inspector-contract/build_preset_release_gcc/bin/pgbuf_inspector_fixture')
env=dict(os.environ,CUBRID=str(install),CUBRID_DATABASES=str(root),CUBRID_TMP=str(root),CUBRID_CONF_FILE=str(root/'cubrid.conf'),LD_LIBRARY_PATH=str(install/'lib')+':'+str(install/'cci/lib'))
env['PATH']=str(install/'bin')+':'+env['PATH']
(root/'databases.txt').touch()
(root/'cubrid.conf').write_text('[common]\ndata_buffer_size=64M\nlog_buffer_size=4M\nvacuum_log_block_pages=4\nenable_pgbuf_inspector=yes\n')
with (root/'native.log').open('wb') as log:
    subprocess.run(['cubrid','createdb','--db-volume-size=20M','--log-volume-size=20M','-F',str(root),'pgtimeout','en_US.utf8'],env=env,cwd=root,stdout=log,stderr=log,timeout=60,check=True)
    child=subprocess.Popen([str(fixture),'pgtimeout'],env=env,cwd=root,stdin=subprocess.PIPE,stdout=log,stderr=log)
    try:
        deadline=time.monotonic()+10
        while b'PGFIXTURE ' not in (root/'native.log').read_bytes():
            assert time.monotonic()<deadline and child.poll() is None
            time.sleep(.02)
        child.stdin.write(b'held')
        child.stdin.flush()
        started=time.monotonic()
        code=child.wait(timeout=14)
        elapsed=time.monotonic()-started
        assert code==1 and 9<=elapsed<14
        assert not list((root/'pgbuf-inspector').glob('*.sock'))
        result=dict(case='incomplete-controller-command',result='PASS',exit_code=code,elapsed=elapsed,evidence=str(root))
        (base/'release-command-timeout.json').write_text(json.dumps(result,indent=2)+'\n')
        print(json.dumps(result))
    finally:
        if child.poll() is None:
            child.kill()
            child.wait()
