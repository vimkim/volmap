# 04: 4,096 runtime 결과의 브라우저 성능과 접근성 검증

**What to build:** 사용자가 밀집된 Volume에서 연속 갱신·mode 전환·scroll·드릴다운을
해도 입력 반응, focus 및 설명이 유지되고 탭별 메모리가 기존 예산 안에 머문다.

**Blocked by:** 03 — 다중 탭과 지연 응답에서도 관찰 수명과 제한 유지.
Lifecycle 오류로 갱신을 생략한 결과를 빠른 rendering으로 측정하지 않기 위해 필요하다.
**External prerequisites:** 설치된 Chromium/Firefox와 재현 가능한 측정 host;
기존 접근성/전용 host 승인 gate에 사람의 검토가 필요한 경우 그 검토도 완료 전 필요하다.
Local harness 작성과 진단은 외부 검토 없이 먼저 진행할 수 있다.05는 blocker가 아니다.

**Status:** ready-for-agent

## Context

[구현 명세](../spec.md)의 browser budget을 새 크기에서 측정하고 실패한 실제
사용자 경로를 최적화한다. 기존12,288-cell density 및 accessibility workload를
유지하고4,096개 변화하는 runtime 증거가 추가된 비용을 검증한다.

## Acceptance criteria

- [ ] Chromium과 Firefox 각각 최소10,000 actual rendered cells를 검증하며
  기존12,288-cell workload를 유지한다. 별도로64 queried sectors/4,096 runtime
  pages와 viewport 안의 수를 기록한다. 가시성 요구를 fixture와 viewport로 충족한다.
- [ ] Default2초 cadence에서 연속 capture의 residency/LRU 값을 실제로 바꾼다.
  동일 capture 반복이나 대부분 unknown인 모델로 실제 갱신 비용을 대신하지 않는다.
- [ ] 수신→decode→채택→paint, input-to-visible, state/LRU 전환, scroll/resize,
  상세표 개폐 및 focus 이동을 측정한다. 각 browser p95 input-to-visible≤100ms다.
- [ ] Matched-disabled 대비 browser-process-tree incremental peak RSS가 탭당
  32MiB 이하다. JS heap만으로 대체하지 않고 allocation·DOM·process RSS를 구분한다.
- [ ]1/8/32탭에서 request 수·admission/거부·표시 age·응답 bytes와 tab visibility를
  기록한다. Hidden 탭은 무요청이어야 한다.1탭 foreground 수치를32개의 active
  관찰자로 잘못 보고하지 않는다. Browser grouping과 RSS 분배 방법을 명시한다.
- [ ] Unknown/stale/paused/expired/absent/refused, Volume 경량 legend/table,
  Sector 상세, non-color 설명과 키보드 focus 유지의 접근성 회귀가 통과한다.
- [ ] 성능 실패 시 실제 browser path를 최적화하되64섹터·정보·cadence·budget을
  줄이지 않는다. 변경 후 적절한 동작 회귀와 영향을 받은 측정을 재실행한다.
- [ ] 명세의 paired-run/warmup/measurement 방식으로 환경·browser version·source
  revision/dirty diff·fixture hash·raw samples·per-case 결과를 남긴다. Browser 또는
  mode별 실패를 평균으로 숨기지 않고 미실행/불확실 결과는 open/inconclusive로 기록한다.
- [ ] Frontend artifacts, 관련 Rust/frontend gates와 repository 검증을 최종 코드에서
  수행한다.05 이후 공통 runtime 코드가 바뀌면 영향 범위의 결과를 재검증한다.

## Verification and handoff

Browser 성능 완료 보고에는 실제 page 수, 탭별 활성/hidden 상태, response adoption
횟수, capture 변화와 raw timing을 포함한다. Missing host/manual prerequisite를
skip-green으로 바꾸지 않는다.05의 서버 자원 시험과 독립적으로 진행 가능하다.

## Comments

2026-09-11 — 기존 density 결과는512-page admission 기준이므로 이 티켓의 증거로 재사용하지 않는다.

2026-09-11 — 사용자 확인 후 로컬 트래커에 게시. 이 상태는 실행 준비를 뜻하며 구현 완료를 뜻하지 않는다.
