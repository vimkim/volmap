# 05: 실제 producer와1/8/32탭 runtime 자원 예산 검증

**What to build:** 운영자가4,096페이지 Volume 관찰을 사용해도 producer와 broker가
기존 CPU·메모리·지연시간 예산을 지키며 과부하·partial 상황을 사실대로 표시한다.

**Blocked by:** 03 — 다중 탭과 지연 응답에서도 관찰 수명과 제한 유지.
새 크기의 올바른 mixed-demand workload가 정량 검증의 선행 조건이다.
**External prerequisites:** Identity가 일치하는 실제 CUBRID producer와 대응 volume,
고정 build/commit,512MiB/1GiB/4GiB pool과 reference workload를 실행할 재현 가능한
측정 환경. Controlled run을 방해하는 다른 부하가 없는 실행 창이 필요하다.
Harness와 local fixture 검증은 먼저 진행할 수 있다.04는 blocker가 아니다.

**Status:** ready-for-agent

## Context

[구현 명세](../spec.md)의 기존 resource gates를 actual changed consumer에서
검증한다. Fixture로 protocol/lifecycle을 증명한 결과와 실제 producer 부하 결과를
구분한다. Producer 기능 개발은 범위 밖이며 기존 budget 실패를 한도 완화로 닫지 않는다.

## Acceptance criteria

- [ ]1/8/32개 동시 및 시차 관찰자에서 실제 HTTP scope/body/response bytes,
  capture identity/scan 요청 수, cached/refresh latency와 endpoint별 오류를 기록한다.
  Volume4,096/Sector64/selected Page 혼합 cadence도 포함한다.
- [ ] Request64KiB/response1MiB와128MiB accounted broker memory를 지킨다.
  Retained+assembly+concurrent responses의 capacities/lifetimes와 actual serialized
  envelope를 측정한다. 기존2MiB response 예약은 충분하다는 증거가 있어야 한다.
- [ ] Cached HTTP p95≤25ms, idle/read-heavy1GiB reference refresh p95≤250ms,
  동시 disk-inspection p95 regression≤5%다. Scan과 lookup/lock/serialization/transfer
  시간을 분리하여 shared scan이 탭별 비용을 제거하는 것으로 보고하지 않는다.
- [ ] Producer CPU≤20%, broker CPU≤50% of one logical core. Matched-disabled
  incremental peak RSS producer≤16MiB/broker≤192MiB이며 allocation128MiB는 별도다.
- [ ]512MiB/1GiB/4GiB pool의 idle/read-heavy/write-flush-heavy/churn workload를
  검증한다. Idle/read-heavy1GiB reference는 complete scans≥99%, 큰 pool은 partial
  사실성과 cap을 확인한다. Fast-but-empty 결과로 usefulness gate를 대신하지 않는다.
- [ ] Paired active-observation throughput loss≤2%, p99 transaction-latency
  increase≤5%를95% confidence upper bound로 평가한다. Parameter-off/unmodified 및
  enabled-no-demand overhead를 별도 기록한다.
- [ ] Stalled producer, overload, 느린 body 소비, pause/hidden/restart를 부하 중
  실행한다. 명시적429/timeout 및 unknown/expiry를 확인하고 disk 서비스는 유지한다.
- [ ] Performance case당10 paired runs,60초 warmup,최소5분 측정과 해당 시
  100,000 transactions를 수행한다. Environment/build/source/fixture/seeds/concurrency,
  raw samples와 confidence calculation을 재현 가능하게 보존한다.
- [ ] 실패한 consumer 경로는 기존 계약 안에서 수정 후 관련 회귀와 성능을 다시
  검증한다. Producer 자체의 결함/환경 부족은 명시적 외부 blocker로 남긴다.
- [ ] 최종 코드의 artifacts와 repository gates가 통과하고 모든 결과가 해당
  revision에 연결된다.04와 코드가 달라졌다면 영향을 받는 결과를 재검증한다.

## Verification and handoff

측정이 오래 걸리면 workload별 checkpoint와 재개 가능한 commands/raw evidence를
남겨 새 세션에서도 이어갈 수 있게 한다. 게이트는 하나의 run으로 평균내지 않는다.
이 티켓과04를 모두 완료해야 전체 성능 검증이 완료되며 어느 쪽도 다른 쪽의
미실행 결과를 대신하지 않는다. 실제 producer prerequisite가 없는 상태의
fixture-only 완료를 전체 ticket 완료로 표시하지 않는다.

## Comments

2026-09-11 — 기존 budget은 유지한다. Producer 스캔 확대 없이 consumer scope만
커졌다는 설계 사실은 성능이 같다는 증거가 아니므로 실제로 측정한다.

2026-09-11 — 사용자 확인 후 로컬 트래커에 게시. 이 상태는 실행 준비를 뜻하며 구현 완료를 뜻하지 않는다.
