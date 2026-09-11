# 03: 다중 탭과 지연 응답에서도 관찰 수명과 제한 유지

**What to build:** Volume/Sector/Page를 여러 탭에서 함께 사용해도 pause, resume,
expiry, restart와 scope가 독립적으로 유지되고 overload가 disk 탐색을 막지 않는다.

**Blocked by:** 02 — 같은 캡처의64섹터 residency/LRU를 Volume에 동시 표시.
새 Volume 및 Sector 요청의 완전한 흐름이 있어야 혼합 demand를 검증할 수 있다.

**Status:** done

## Context

[구현 명세](../spec.md)의 lifecycle/admission 계약을 새 두 projection과 혼합
탭에서 adversarial하게 검증하고 발견된 결함을 수정한다.01/02에서 기본 수명
보장을 미루는 티켓이 아니라 경계 상황을 닫는 사용자 동작 slice다.

## Acceptance criteria

- [x]1/8/32탭의 동시 및 시차 demand를 실제 HTTP 경로로 재현한다. 허용된
  scope가 공유 scan을 사용하면서 각자의 scope/cadence/epoch를 유지한다.
- [x]8 observation admission, 추가 queue 없음,2.5초 deadline을 유지한다.
  초과 demand는429, 느린 응답 body가 살아 있는 동안 permit/charge 유지,
  취소·timeout 뒤 반환을 검증한다.32탭 전체의 무거부 성공을 요구하지 않는다.
- [x] Metadata4개 별도 admission/1초 deadline, disk admission 독립성,
  stalled producer·과부하 중에도 disk navigation이 동작함을 보인다.
- [x] Pause는 신규 page 증거를 채택하지 않고5초 metadata만 확인한다.
  다른 탭의 새 capture는 availability로만 보이며30초 expiry는 pause 중에도 작동한다.
- [x] Resume 이후 시작된 scan만 채택한다. Resume 이전 시작·이후 완료된
  in-flight scan을 거부하며 broker500ms scan-start floor를 지킨다.
- [x] Hidden 상태에는 요청이 없고 복귀 시 fresh demand를 만든다. Route/scope,
  generation, pause/overlay, visibility 변화 직전 발송한 지연 응답을 채택하지 않는다.
- [x] Restart/identity mismatch는 runtime evidence를 제거하고 명시적 retry가
  복구한다. Disk 탐색·observed disk state·inspection coverage는 유지한다.
- [x] Conservative age와 caller별 fresh threshold, 동일 capture age 단조성,
  clock step/suspension의 불확실성,30초 expiry와 기존 retry/jitter를 검증한다.
- [x] 한 탭 취소는 다른 탭 refresh를 죽이지 않고 마지막 관찰자 취소는
  불필요한 scan을 중단한다. Captures를 누적하여 partial absence를 만들지 않는다.
- [x] Retained capture+in-flight assembly+concurrent responses의 최대 생존
  조합을 시험하고128MiB accounting 이내 또는 명시적 bounded 거부를 증명한다.
- [x] HTTP capture/request trace와 browser 사용자 상태를 함께 남긴다. 정밀한
  시간 경계에는 기존 virtual scheduler를 사용하고 sleep에 의존한 flaky assertion을 피한다.

## Verification and handoff

기존 broker lifecycle 및 browser real-producer restart/multi-tab 회귀를 재사용한다.
이 티켓은 local authenticated fixture로 완료할 수 있다. 실제 producer의 부하
측정과 browser 정량 성능은04/05에 남기고 여기의 통과로 대체하지 않는다.
성능 측정용 혼합 scope/cadence 및 과부하 재현 workload를 후속 티켓에 전달한다.

## Comments

2026-09-11 — 전 탭이 같은 capture를 받는다고 가정하지 않는다. 한 응답 내 동일
capture 보장과 demand별 cache/refresh 선택을 별도로 assertion한다.

2026-09-11 — 사용자 확인 후 로컬 트래커에 게시. 이 상태는 실행 준비를 뜻하며 구현 완료를 뜻하지 않는다.

2026-09-11 — 구현 및 Standards/Spec 검토 완료. `just verify` 통과 (browser 93 passed, 1 skipped). [검증 기록](../verification/03-lifecycle/README.md)에 acceptance 매핑과 HTTP/browser trace를 기록했다. 실제 producer의 100 ms partial scan은 별도 진단으로 남으며 이 lifecycle 완료가 실제 부하 성능 통과를 뜻하지 않는다.
