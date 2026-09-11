# View-scoped runtime observations: implementation specification

**Status:** ready-for-agent

**Blocked by:** None. Execution prerequisites are recorded in the individual tickets.

**Approved:** 2026-09-11 — user confirmed the existing HTTP/browser and virtual-clock test seams and five-ticket breakdown.

## Problem Statement

Volume에서 여러 섹터를 비교할 때 현재 512페이지 순환 조회는 한 번에8섹터의
runtime 증거만 남긴다. 사용자는 적어도64개의 가시 섹터를 같은 공유 캡처로
비교하고 싶다. 상세 Page용 증거를4,096개 전송하면 기존 HTTP byte 한도를
넘고 탭별 처리 비용이 커진다. 서로 다른 캡처를 합치면 증거 시점과 partial
unknown 의미가 흐려진다.

## Solution

Volume은 중앙에 가까운 최대64개 가시 섹터의4,096페이지에 대해 residency와
LRU 정보를 동시에 표시한다.64개 이하는 전부 선택하고 초과분은 미조회로
명시한다. 시간에 따라 선택을 순환하지 않으며 scroll/resize로 범위가 바뀔
때만 재선택한다. Sector는 해당64페이지의 기존 상세 runtime 정보를 유지한다.

요청은 volume와 sector 단위로 표현한다. Volume 응답은 최소 필드만 포함하고,
하나의 응답은 하나의 공유 캡처를 사용한다. Runtime observation은 observed
disk state와 독립적이며 partial unknown, freshness, pause/resume/restart의
기존 의미와 자원 한도를 유지한다.

## User Stories

1. As a Volume 사용자, I want 64개 가시 섹터의 관찰 결과를 동시에 보기, so that8섹터 순환을 기다리지 않고 비교할 수 있다.
2. As a Volume 사용자, I want 64개 이하의 가시 섹터는 모두 포함하기, so that 불필요한 미조회가 생기지 않는다.
3. As a Volume 사용자, I want 64개 초과 시 중앙 우선으로 안정적인 범위를 선택하기, so that 관심 영역의 색상이 시간 순환으로 사라지지 않는다.
4. As a Volume 사용자, I want 초과분의 미조회를 명시하기, so that 증거 부재를 nonresidency로 오해하지 않는다.
5. As a Volume 사용자, I want scroll/resize에 따라 범위를 갱신하기, so that 현재 보는 섹터를 조사한다.
6. As a Volume 사용자, I want 같은 선택 집합의 순서 변화에는 증거를 유지하기, so that 작은 화면 이동이 불필요한 재조회를 만들지 않는다.
7. As a Volume 사용자, I want resident/not-resident/unknown 및 이유를 구분하기, so that 관찰의 한계를 알 수 있다.
8. As a Volume 사용자, I want LRU zone과 list kind/index를 보기, so that residency와 LRU 소속을 함께 비교할 수 있다.
9. As a Volume 사용자, I want resident지만 LRU가 unknown인 상태도 표현하기, so that 별개의 사실을 하나로 단정하지 않는다.
10. As a Volume 사용자, I want unknown index와 membership 없음 및 index0을 구분하기, so that 존재하지 않는 LRU 소속을 만들지 않는다.
11. As a Volume 사용자, I want 전체 표시 결과가 같은 capture identity를 갖기, so that 서로 다른 시점의 증거를 합친 것으로 오해하지 않는다.
12. As a Volume 사용자, I want 캡처가 원자적 상태가 아니라는 설명을 보기, so that 실제보다 강한 일관성을 가정하지 않는다.
13. As a Volume 사용자, I want 간결한 legend/tooltip/상세표, so that 조회하지 않은 dirty/flushing 정보가 false나 undefined로 나타나지 않는다.
14. As a Sector 사용자, I want sector 하나를 지정해64페이지의 기존 상세 증거를 보기, so that Volume 경량화로 드릴다운 정보가 줄지 않는다.
15. As a selected Page 사용자, I want 기존 상세 관찰 동작을 유지하기, so that 이 변경이 페이지 진단을 방해하지 않는다.
16. As a 사용자, I want partial capture의 누락을 unknown으로 보기, so that 불완전한 스캔을 부재 증거로 해석하지 않는다.
17. As a 사용자, I want duplicate VPID와 unevaluated를 구분하기, so that 모호함과 미조회를 진단할 수 있다.
18. As a 사용자, I want requested/evaluated와 producer completeness를 따로 보기, so that 조회 완료를 스캔 전체 완료로 오해하지 않는다.
19. As a 사용자, I want stale 표시와30초 expiry를 유지하기, so that 오래된 색상을 최신 상태로 믿지 않는다.
20. As a 사용자, I want 동일 capture 재조회가 나이를 갱신하지 않기, so that cache 사용이 freshness를 과장하지 않는다.
21. As a paused 사용자, I want 신규 증거 채택이 멈추되 만료와 identity 무효화는 계속되기, so that 정지 화면의 증거 한계를 유지한다.
22. As a paused 사용자, I want 다른 탭의 새 capture를 availability로만 알기, so that 다른 탭이 내 화면을 resume하지 않는다.
23. As a resumed 사용자, I want resume 이후 시작한 scan의 증거를 받기, so that pause 이전 in-flight 결과가 새 관찰로 채택되지 않는다.
24. As a hidden-tab 사용자, I want 불필요한 요청이 중단되고 복귀 시 유효한 증거를 받기, so that 백그라운드 부하와 오래된 표시를 줄인다.
25. As a 사용자, I want restart 시 runtime 증거만 제거하고 명시적 retry로 복구하기, so that disk 탐색을 잃지 않는다.
26. As a 다중 탭 사용자, I want scan을 공유하면서 각 scope와 cadence를 존중하기, so that 다른 탭이 내 증거 의미를 바꾸지 않는다.
27. As a 다중 탭 사용자, I want overload를 명시적으로 받고 disk 탐색은 유지하기, so that 많은 탭이 기본 inspector를 막지 않는다.
28. As a 사용자, I want 범위 변경 뒤 늦게 도착한 응답을 무시하기, so that 다른 volume/sector/generation의 색상을 보지 않는다.
29. As a 키보드 사용자, I want 갱신 중 focus와 비색상 설명을 유지하기, so that 색상이나 마우스에 의존하지 않는다.
30. As an 운영자, I want 요청·응답·메모리·CPU가 기존 예산을 지키기, so that 표시 범위 확대가 예측 불가능한 부하를 만들지 않는다.
31. As a 검증자, I want 실제4,096개의 변화하는 결과와1/8/32탭을 측정하기, so that 작은 fixture의 통과를 전체 성능으로 오인하지 않는다.
32. As a 검증자, I want raw samples와 실행 환경 및 실패·미측정을 보존하기, so that 설계 가능성과 실제 통과를 구분한다.

## Implementation Decisions

1. Scope addressing과 응답 projection을 분리한다. Sector 상세 조회를 최초 수직
   slice로 연결하면서 view-scoped 요청을 추가하고, 이어 Volume 경량 조회를
   연결한다. Shared broker/HTTP/client decode/view의 기존 경계를 사용한다.
   필요한 국소 정리는 해당 slice에 포함하고 별도 광범위 refactor를 만들지 않는다.
2. Volume scope는 volume ID와 중복 없는 sector ID 최대64개다. 중앙 거리와
   sector ID로 선택한 뒤 ID 정렬 순서로 canonicalize한다. Sector scope는
   volume ID와 sector ID 하나다. 비가시 영역을 채우거나 레이아웃을 바꾸지 않는다.
3. Scope는 요청 generation의 inspection projection으로 검증한다. 음수·범위
   초과·중복·존재하지 않는 식별자와 overflow를 명시적으로 거부한다. 짧은 마지막
   sector의 유효하지 않은 page slot은 식별 가능하게 남기고 뒤 주소를 당기지 않는다.
   유효 물리 페이지만 requested/evaluated에 센다. 읽을 페이지가 없으면 frontend는
   관찰 요청을 보내지 않는다. 검증은 디스크 바이트 재해석이나 runtime page load를 하지 않는다.
4. Volume은 state/reason과 resident의 LRU zone, list kind, kind-local index만
   받는다. unknown/none/zero 및 residency와 LRU 가용성의 독립성을 보존한다.
   dirty/flushing/latch/fix/LSA는 Volume에 포함하지 않는다. Sector는 기존 상세를
   유지한다. 새 payload는 기존 명시적 VPID payload와 구별되는 schema/variant로
   decode하고 기존 selected-page 경로와 상한은 유지한다.
5. 응답은 scope를 echo하고 sector/page-slot 순서로 결과를 반환한다. 공통
   capture identity, incarnation, age, topology, coverage, capability 및 limitations는
   envelope에 한 번 둔다. Client는 scope/epoch/generation/slot 구조를 검증하고
   전체 batch를 원자적으로 채택한다. 이것은 producer scan의 원자성을 뜻하지 않는다.
6. 하나의 응답은 하나의 검증된 capture를 조회한다. 여러512페이지 응답의 합집합,
   다른 selected-page capture와의 합성, capture 간 누적 nonresidency 추론은 없다.
   Producer protocol, scan 범위와 cap은 확대하지 않는다.
7. Valid footer가 있는 complete capture의 누락만 not-resident다. Partial omission과
   duplicate는 unknown이고 미조회는 unevaluated다. Malformed capture는 publish하지
   않으며 이전 증거도 유효한 경우에만 유지한다. evaluated는 확정 상태 수가 아니다.
8. Volume/Sector cadence2초, selected Page500ms, broker scan-start floor500ms를
   유지한다. Cache reuse는 caller cadence를 따른다. Fresh는 conservative upper age가
   cadence의2배 이내인 경우뿐이며 cache hit로 나이를 갱신하지 않는다. 왕복 시간과
   browser 경과 시간을 포함하고 불확실한 age 및30초 expiry에서 증거를 제거한다.
9. Pause는 채택 정지다. Metadata는5초 주기로 확인하고 hidden tab은 요청하지 않는다.
   Resume와 visibility 복귀는 demand 이후 시작한 scan을 요구한다. Scope, epoch,
   generation, overlay, visibility, pause 및 incarnation 변화의 지연 응답을 차단한다.
   Restart/identity mismatch는 runtime 증거를 제거하고 기존 명시적 retry로 복구한다.
10. 요청64KiB/응답1MiB, broker accounted memory128MiB, decoded-scan48MiB 이하,
    observation admission8개·추가 queue 없음·deadline2.5초를 유지한다. Capability는
    별도4개·1초다. 초과 demand는 명시적 overload를 받고 disk admission은 독립적이다.
    Body 소유자가 해제될 때까지 response charge/permit을 유지한다. 기존 요청별2MiB
    예약이 새 자료구조의 capacities/lifetimes를 포함하는지 검증하고 실제보다 작게 예약하지 않는다.
11. Producer65,536 slots/records,64MiB scan,4KiB frames,100ms traversal budget,
   2초 exchange와 기존 bounded buffering/handshake 보장을 유지한다. 현 capture32MiB
    예약 구현과 허용48MiB cap을 혼동하지 않는다. 전체 admission은 개별 cap보다 먼저
    거부할 수 있다. RSS와 allocation accounting은 별개의 수치다.
12. Retry는 기존0.5/1/2/4/8초 및±20% jitter, 최대9.6초다. 마지막 관찰자 취소만
    불필요한 refresh를 멈추며 다른 탭의 demand는 취소하지 않는다.
13. Volume legend/tooltip/상세표는 경량 projection 전용이다. Sector와 Page의
    상세 표시는 유지한다. Fresh/stale/expired/unknown을 비색상 표현으로도 제공하고
    갱신 시 focus를 보존한다. 접힌 상세표와 unchanged-map 재사용의 기존 최적화를 유지한다.
14. Legacy collection512 제한과 producer wire v1은 그대로 두고 Volume observation
    admission만4,096으로 확대한다. 기존 자원 예산 결정의512페이지 부분만 이 명세와
    accepted ADR이 개정한다. 완료에는 frontend 배포용 artifact 재생성과 표준 검증이 포함된다.

## Testing Decisions

주 검증 경계는 **실제 HTTP를 거치는 사용자 관찰 흐름**이다. 기존 fixture producer와
HTTP server를 사용하여 request부터 normalized response까지 시험하고 기존 browser
suite로 실제 request와 화면 채택을 확인한다. 시간 경계·취소·race는 기존 virtual
clock/scheduler broker 시험을 보조 경계로 사용한다. 새로운 test-only 서비스나
UI에 주입한 대체 모델을 주 증거로 만들지 않는다. 이 테스트 경계는2026-09-11 사용자 확인을 받았다.

- 좋은 시험은 외부에서 보이는 주소, 결과 분류, scope/capture identity, HTTP 거부,
  화면 설명과 수명 보장을 확인한다. 내부 helper 호출 횟수나 구현 구조 복제는 피한다.
  Scan 공유는 producer가 받은 요청과 공개 capture identity로 검증한다.
- Prior art: 현재 HTTP scope/body-limit 및 concurrent-scopes 시험, broker의 caller
  cadence/resume/expiry/cancellation/restart 시험, browser의 overlay accessibility,
  real-producer multi-tab/restart 시험과12,288-cell density 측정을 확장한다.
- Sector1개/Volume0·1·63·64·65개, 중복·비연속·최대 ID·overflow·짧은 마지막
  sector를 검증한다. 총4,095/4,096 유효 페이지와4,097을 요구하는 잘못된 범위를
  구분한다. 최대 폭 resident/LRU와 unknown/null 필드 및 모든 공통 metadata를
  포함한 실제 serializer 출력으로 byte limit을 시험한다.
- 64개의 완전한 sector와4,096개 변화하는 resident/LRU 결과를 같은 capture로
  표시하는 browser fixture를 만든다.64개 초과는 고정 선택·미조회, scroll/resize는
  재선택, 동일 선택 집합의 재정렬은 batch 유지인지 확인한다.
- Complete/partial/duplicate/unevaluated/malformed/absent/refused/incompatible를
  각각 검증한다. 새로운 요청 shape에서도 old capture + assembly + responses가
  함께 살아 있는 동안 accounting cap을 확인한다.
- 1/8/32개 탭의 동시·시차 demand, 느린 body 소비, 취소, stalled producer,
  overload와 disk 요청을 함께 시험한다.8개보다 많은 탭의 무거부 서비스나
  starvation-free 스케줄링은 약속하지 않는다. 허용된 요청의 결과와 거부를 모두 기록한다.
- Pause 중 expiry/restart/다른 탭 refresh, hidden 복귀, resume 이전 in-flight scan,
  generation/route 변경, clock step/suspension을 검증한다. Volume 경량 응답과
  Sector/Page 상세 응답의 혼합 탭에서도 scope와 caller cadence가 독립적이어야 한다.

성능은 아래 gate를 **측정 후** 판정한다. Size model이나 현재 작은 응답의 green
결과로 대체하지 않는다. 이미 있는 외부 검증 prerequisite는 해당 ticket에 유지한다.

| 대상 | Gate와 측정 범위 |
| --- | --- |
| Chromium/Firefox | 각각 input-to-visible p95≤100ms, matched-disabled 대비 탭당 peak RSS 증가≤32MiB. 최소10,000 actual rendered cells(기존12,288 유지),4,096개 변화하는 runtime 결과, state/LRU, scroll/resize/상세표·focus·연속 갱신. 수신→decode→채택→paint와 전송 bytes도 기록한다. |
| Runtime service | Default cadence,1/8/32탭. Cached HTTP p95≤25ms, reference refresh p95≤250ms, 동시 disk-inspection p95 regression≤5%. 요청별 직렬화·lock 대기·전송과 shared scan 비용을 구분한다. |
| CPU/RSS | Producer CPU≤20%, broker≤50% of one logical core. Incremental peak RSS producer≤16MiB, broker≤192MiB. Broker allocation128MiB 한도는 별도로 증명한다. |
| Real producer |512MiB/1GiB/4GiB pool, idle/read-heavy/write-flush-heavy/churn. Idle/read-heavy1GiB reference complete scans≥99%; 큰 pool은 truthful partial을 확인한다. Throughput loss≤2%, p99 transaction latency increase≤5%; disabled/no-demand를 별도 측정한다. |
| Evidence quality | Performance case당10 paired runs,60초 warmup·최소5분 measurement·해당 시100,000 transactions까지 연장.95% confidence upper bound로 overhead gate를 평가하고 불확실하면 inconclusive. 실행 hardware/build/commits/dirty diff/fixture/seeds/concurrency/raw samples/method를 남긴다. |

## Out of Scope

- 전체 volume의 무제한 runtime 조회, 화면 레이아웃 변경, 시간 순환 또는 비가시 영역 채우기.
- Producer 스캔 cap 확대, producer 기능 개발, 여러 캡처의 합성·누적 history.
- Volume의 dirty/flushing/latch/fix/LSA 표시와 selected-page 진단 범위 확대.
- Runtime을 inspection graph, coverage, revision, export 또는 TUI 계약에 편입하기.
- 인증·listener 정책 변경, raw page/application payload 노출, 실제 성능 gate의 임의 완화.
- 이번 명세/티켓 작성 단계에서의 기능 구현 또는 기존 다른 작업의 변경.

## Further Notes

제품 정책, test seams와 실행 티켓의 크기·blocking edges는 사용자 확인을 받았다.
기존 working-tree 변경을 보존하고
각 구현 시작 시 현재 revision과 변경 baseline을 기록한다.

Authoritative context: accepted view-scoped-runtime ADR, runtime capability ADR,
lightweight-observation/resident-inspection separation ADR, 기존 overlay resource-budget
결정 및64-sector 설계 조사. 원문 위치와 최신 코드 탐색은 [companion review](review.md) 문서에 둔다.
이 구현 명세는 historical design spec과 완료된 decision map을 수정하지 않는다.

경량 size model의 resident body는514,756bytes(공통 metadata 제외)다. 이는 설계
가능성 근거이며 실제 serializer/메모리/지연시간 gate의 통과를 뜻하지 않는다.
티켓01→02→03 후04와05는 서로 독립적으로 진행할 수 있다. 전체 완료는 모든
acceptance criteria와 gate가 충족되고 해당 결과가 기록된 상태다. 외부 측정
환경이 없으면 runnable harness까지 진행하되 미실행 gate를 완료로 표시하지 않는다.
