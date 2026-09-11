"""Rebuild the ticket gate manifest from retained test logs and final binaries."""
from pathlib import Path
import hashlib
import json
import re
import subprocess
import xml.etree.ElementTree as ET

root = Path('/home/vimkim/gh/cb/CBRD-27398-pgbuf-inspector-contract')
out = Path(__file__).resolve().parent

def git(*args):
    return subprocess.check_output(['git', '-C', str(root), *args], text=True).strip()

def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

manifest = {
    'ticket': '04-prove-overload-and-lifecycle-isolation',
    'source_commit': git('rev-parse', 'HEAD'),
    'tested_tree': git('rev-parse', 'HEAD^{tree}'),
    'reviewed_tree': git('rev-parse', 'HEAD^{tree}'),
    'baseline': '3ae2404dc7832243ca9be7dc160768222d0594b4',
    'contract': json.loads((root / 'docs/pgbuf-inspector/v1/manifest.json').read_text()),
    'environment': json.loads((out / 'environment.json').read_text()),
    'modes': {},
    'consumer_commit': None,
    'external_testcase_revision': None,
    'scope': 'Producer ticket 04 on the format-aligned branch; no engine source changes in this ticket.',
    'reviews': json.loads((out / 'review.json').read_text()),
    'earlier_attempts': [
        {'log': 'socket-first.log', 'status': 'failed-precondition',
         'reason': 'Broad [socket] filter included hidden credential case without UID isolation; corrected filter and separate isolated run.'},
        {'log': 'debug-server-first.log', 'status': 'inconclusive',
         'reason': 'String-compressed SQL fixture did not saturate output; explicit uncompressed disposable fixture established large capture.'},
        {'log': 'release-server-first.log', 'status': 'inconclusive',
         'reason': 'Socket-diagnostic output column parsing did not identify two sockets; corrected from actual ss output.'},
        {'log': 'release-suite.log', 'status': 'build-not-started',
         'reason': 'Local installer refused replacement while this task real-server harness was running; retried after harness cleanup.'},
        {'log': 'release-suite-final.log', 'status': 'failed-and-terminated',
         'reason': 'Long default PL socket path caused Java startup failure; terminated this task CTest and child, reran with short CUBRID_TMP.'},
        {'log': 'debug-suite-final.log', 'status': 'failed-environment',
         'reason': 'Concurrent mode suites share the OOS unittestdb fixture; serialized reruns avoid DB contention.'},
    ],
    'limitations': [
        'Native no-wait argument is source audit of trylock/unlock plus sampling/cancellation tests and real SQL/shutdown; no controlled native held-mutex experiment claimed.',
        'Queue test accounts pending wire bytes separately from kernel send memory; it is not allocator/RSS/performance evidence.',
        'External shell repository, develop port, consumer integration, controlled dirty/eviction and dedicated-host release performance remain their owning later gates.',
        'Binaries were built before the final commit amendment; final source content matches the tested/reviewed tree. Binary hashes are recorded independently.'
    ],
}
for mode in ['debug', 'release']:
    suite = out / f'{mode}-suite-verified.log'
    text = suite.read_text()
    assert '100% tests passed, 0 tests failed out of 27' in text, suite
    counts = re.search(r'All tests passed \((\d+) assertions in (\d+) test cases\)', text)
    assert counts, suite
    credential_log = out / ('debug-credentials-final.log' if mode == 'debug' else 'release-credentials.log')
    credential = re.search(r'All tests passed \((\d+) assertions in (\d+) test cases\)', credential_log.read_text())
    assert credential
    socket_log = out / ('debug-sockets-final.xml' if mode == 'debug' else 'release-sockets.xml')
    xml = ET.parse(socket_log).getroot()
    assert int(xml.find('OverallResults').get('failures')) == 0
    sockets = [case.get('name') for case in xml.findall('.//TestCase')]
    artifacts = out / f'{mode}-server-artifacts'
    isolation = json.loads((artifacts / 'isolation.json').read_text())
    assert isolation['stalled_clients'] == 2
    for phase in ['saturation_before', 'saturation_after']:
        assert len(isolation[phase]) == 2
        assert all(row['charged'] >= row['budget'] > 0 for row in isolation[phase])
    server_log = out / ('debug-server-verified.log' if mode == 'debug' else 'release-server-final.log')
    server_text = server_log.read_text()
    assert 'Traceback' not in server_text
    assert 'PASS removed conflict does not trigger a background activation retry' in server_text
    manifest['modes'][mode] = {
        'ctest_passed': 27, 'ctest_failed': 0,
        'inspector_cases': int(counts[2]), 'inspector_assertions': int(counts[1]),
        'credential_cases': int(credential[2]), 'credential_assertions': int(credential[1]),
        'socket_cases': len(sockets), 'socket_case_names': sockets,
        'server_checks': sum(line.startswith('PASS ') for line in server_text.splitlines()),
        'isolation': isolation,
        'logs': {'suite': str(suite), 'credentials': str(credential_log), 'sockets': str(socket_log), 'server': str(server_log)},
        'binary_sha256': {str(path): digest(path) for path in [root / f'build_preset_{mode}_gcc/bin/{name}' for name in ['cub_server','test_pgbuf_inspector']]},
        'commands': [
            'CUBRID_TMP=' + (out / 'suite-socket-root.txt').read_text().strip() + f' ctest --test-dir build_preset_{mode}_gcc --output-on-failure --verbose',
            f'build_preset_{mode}_gcc/bin/test_pgbuf_inspector "[socket]~[.credential]" -r xml --durations yes',
            f'unshare --map-auto --map-root-user build_preset_{mode}_gcc/bin/test_pgbuf_inspector "[.credential]"',
            'TMPDIR=/home/vimkim/temp python3 unit_tests/pgbuf_inspector/server_attachment.py --scan --isolation (selected installed build environment)',
        ],
    }
manifest['requirements'] = [
    {'requirement': line.split('|')[1].strip(), 'evidence': line.split('|')[2].strip(), 'owner': 'CUBRID producer', 'result': 'passed-debug-and-release'}
    for line in (root / 'docs/pgbuf-inspector/isolation.md').read_text().splitlines()
    if line.startswith('| ') and not line.startswith('| Requirement') and not line.startswith('| ---')
]
manifest['artifact_sha256'] = {str(path.relative_to(out)):digest(path) for path in sorted(out.rglob('*')) if path.is_file() and path.name not in ['manifest.json','checkpoint.md']}
(out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(json.dumps({mode: {key: value for key,value in data.items() if key.endswith('_cases') or key.startswith('ctest_')} for mode,data in manifest['modes'].items()}, indent=2))
