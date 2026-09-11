# Volume overlay: 64섹터 동시 관찰 검토

Status: design-accepted — 2026-09-11 사용자 확인 완료; 구현과 성능 측정은 후속 작업

2026-09-11. 기준 HEAD `5dacafb`와 조사 당시의 기존 working-tree 변경.
이 문서는 신규 설계 조사이며 기존 overlay 결정 기록을 소급 수정하지 않는다.

## 요청과 의미

Volume overlay에서 64섹터·4,096페이지를 같은 공유 캡처에서 조회하여
동시에 표시한다. 부분 스캔의 unknown, pause/restart 의미는 보존한다.
동일 캡처는 동일 producer scan의 증거라는 뜻이며 원자적인 버퍼풀 상태가 아니다.
동시 표시 가능한 범위 확대가 resident 또는 확정 상태 4,096개를 보장하지 않는다.

### 합의된 방향 — 2026-09-11 추가 답변

사용자: “필요한 부분만 조회”, “volume은 4096의 LRU list와 resident 여부”,
“volume level 요청 혹은 sector level 요청으로 설계”.

따라서 **Volume 경량 projection + view level scope 요청**을 채택한다.
아래 full-evidence 크기 계산은 배제한 대안의 비용 근거다. 요청/응답 byte
한도 확대를 진행하지 않고 64KiB/1MiB 안에서 설계·검증한다.

Volume의 정보는 resident/not-resident/unknown 분류와 이유, LRU zone,
list kind 및 kind-local index다. 기존 색상은 zone을 사용하고 private
membership을 별도로 표현하므로 index만으로 현재 LRU 표현을 대체할 수 없다.
dirty/flushing/latch/fix/LSA는 Volume 응답과 상세표에서 제외하며,
legend·tooltip에서도 관찰하지 않은 dirty/flushing을 false처럼 표현하지 않는다.
Sector는 기존 64페이지의 dirty/flushing 등 상세 runtime 정보를 유지한다.
사용자가 Q1/Q2 권장안에 “yes”로 답하여 아래 표시 범위와 상세 수준을 확정했다.

### 요청/응답 설계 계약

```json
{"scope":{"kind":"volume","volid":0,"sector_ids":[0,1,2]},"epoch":"12","generation":"7","cadence_ms":2000,"after_request":false,"retry":false}
```

```json
{"scope":{"kind":"sector","volid":0,"sector_id":12},"epoch":"13","generation":"7","cadence_ms":2000,"after_request":false,"retry":false}
```

- Volume: 최대64개 중복 없는 sector ID. 비연속 가시 섹터를 지정할 수 있다.
  Sector: sector ID 하나. 명칭이 Volume이어도 volume 전체 스캔 요청은 아니다.
- 가시 섹터가64개 이하이면 전부 선택한다. 초과하면 기존 화면 중앙 거리,
  sector ID 우선순위로64개를 선택하며 주기적인 순환은 하지 않는다. 나머지는
  미조회로 명시한다. scroll/resize로 가시 범위가 바뀌면 선택을 다시 계산한다.
  비가시 섹터를64개까지 채우지 않고 레이아웃도 이 작업에서 변경하지 않는다.
- generation에 해당하는 inspection projection으로 volume/sector 존재,
  유효 페이지 범위와 checked arithmetic을 검증한다. UI나 runtime adapter가
  volume bytes를 재해석하지 않는다. 마지막 섹터의 부족한 페이지를 만들어내지 않는다.
- 서버는 scope를 검증한 후 유효한 최대4,096페이지로 확장하고 하나의 capture를
  조회한다. producer protocol과 scan caps는 유지한다. 임의 페이지의 상세 요청은
  기존 selected-page 경로와 구분하고 이 작업으로 상한을 자동 확대하지 않는다.
- 응답은 scope를 echo하고 sector별 page-slot 순서의 결과를 반환한다.
  페이지 주소는 sector/slot에서 복원하여 요청과 응답의 반복 VPID를 줄인다.
  유효하지 않은 slot은 명시적으로 나타내어 뒤 페이지의 위치가 당겨지지 않게 한다.
- capture/capability/age/topology/coverage/limitations는 공통 envelope에 한 번만
  둔다. 브라우저는 scope/epoch/generation/slot 구조를 검증하고 전체 batch를 채택한다.
  기존 VPID 기반 decoder와 구별되는 명시적 schema/variant를 사용한다.
- resident와 LRU 정보의 가용성은 별개다. resident지만 LRU가 불명일 수 있다.
  list index의 unknown과 membership 없음(null)을 유지하고 zero로 대체하지 않는다.
- 선택된 sector ID는 정렬한 canonical 순서로 요청·비교한다. 같은 선택 집합의
  거리 순서만 바뀌어도 batch를 폐기하지 않는다. 실제 범위 변경은 epoch로
  지연 응답을 차단한다.

`size-model.py`를 실행한 compact JSON 모델:

| 64섹터 scope | bytes |
| --- | ---: |
| 최대 폭 sector ID 64개의 요청 | 753 |
| resident + LRU 결과 4,096개 | 514,756 |
| partial unknown 4,096개 | 199,364 |
| not-resident 4,096개 | 240,324 |

응답 수치는 scope와 sector 결과만 포함한다. 공통 metadata까지 포함한 Rust
직렬화 경계 시험은 구현 시 필요하다. 이 결과는 full-evidence 약1.69–2.13MiB보다
작고 기존1MiB cap 안에서 설계할 여지를 보인다. 기존2MiB 요청별 예약을
유지할 수 있는지도 새로운 내부 자료구조의 실제 capacity/lifetime으로 확인한다.
2초 간격의 resident 모델 본문은 탭당 약0.245MiB/s, 8개 탭 약1.96MiB/s다.
스캔 공유로 이 HTTP 트래픽이나 탭별 rendering이 사라지지는 않는다.

## 코드로 확인한 사실

| 항목 | 현재 동작과 근거 |
| --- | --- |
| 8섹터 제한 | `web/src/observations.ts:130`에서 512페이지를 선택하고 초과분을 순환한다. `:161` 부근에서 batch를 통째로 교체한다. 캡처 간 합집합은 없다. |
| 가시 범위 | `web/src/observation-view.tsx:8`의 DOM 가시성 측정으로 가시 섹터를 찾고 화면 중앙 거리, sector ID 순으로 정렬한다. 화면에 64섹터가 보이도록 레이아웃을 바꾸는 기능은 별개다. |
| 다층 상한 | `src/web.rs:314`, `src/web/observations.rs:37`, `web/src/observations.ts:257,281`에 각각 512 제한이 있다. 일반 collection의 512 제한은 별개다. |
| HTTP | `src/web.rs:264`의 공통 body limit은 64KiB. `src/web/observations/session.rs:581,656`의 응답 writer는 1MiB로 제한한다. |
| 공유 조회 | `src/web/observations/session.rs:243` 이후 같은 session lock에서 cache/refresh를 선택하고 `:504`에서 동일 latest capture를 조회한다. `wire.rs:96`의 정렬된 records에 대해 두 번의 binary partition lookup을 수행한다. |
| 다중 탭 | `session.rs:231` 이후 8개 admission, 요청당 2MiB 예약, 2.5초 deadline. 응답 body가 해제될 때 permit과 예약이 반환된다. 스캔은 공유하지만 요청별 조회·직렬화·응답은 공유하지 않는다. |
| 부분 증거 | `session.rs:521` 이후 partial omission과 duplicate는 unknown. complete capture에서만 누락을 not-resident로 분류한다. 범위 밖 volume은 unevaluated이며 evaluated에 포함하지 않는다. |
| cadence | `web/src/observations.ts:315`: Volume/Sector 2초, selected Page 500ms. 응답 채택 후 다음 지연을 예약하므로 실제 주기는 처리 시간도 포함한다. |
| freshness | `observations.ts:156` 이후 upper age + HTTP 왕복 시간. 동일 capture 재수신 시 나이를 되돌리지 않는다. fresh는 요청 간격의 2배 이내, age 불명 또는 30초 도달 시 증거 제거. Volume fresh 기준은 4초다. |
| pause | `observations.ts:121,141,204,224`: 새 응답 채택 금지, 표시 증거는 만료 가능, 5초마다 metadata만 확인. 다른 탭의 새 capture는 newer availability로만 알린다. |
| resume/restart | resume/visibility 복귀는 after_request로 demand 이후 시작한 scan을 요구한다. incarnation 변경은 이전 증거를 제거하고 명시적 retry 경로를 거친다. scope/epoch/generation이 맞지 않는 지연 응답은 채택하지 않는다. |

## 크기와 비용

현재 JSON 모양을 Python compact JSON으로 계산한 값이다. 실제 Rust 직렬화
벤치마크, 압축 후 전송량, 브라우저 heap 또는 지연시간 측정값이 아니다.

| 4,096페이지 모델 | 요청 bytes | pages + observations bytes (공통 metadata 제외) |
| --- | ---: | ---: |
| 페이지 0..4095, 현재 fixture의 resident evidence | 105,479 | 1,770,266 |
| 최대 폭 식별자/현재 알려진 evidence 필드 | 약 147,587 | 2,236,444 |

현재 64KiB/1MiB 한도를 모두 넘는다. 한도만 4,096으로 바꾸는 구현은 불가하다.
2초마다 이 크기의 응답을 받는 이상화된 경우 탭당 약 0.84–1.07MiB/s,
8개 탭은 약 6.75–8.53MiB/s의 JSON 본문이다. 실제 주기는 처리 시간과
admission에 좌우되며 32개 탭에 동시 서비스나 무기아를 보장하지 않는다.

대안 A: 현재 full-evidence 응답 유지, observation 전용 request 256KiB /
response 4MiB 한도 후보. 기존 상세 표시를 유지하기 쉽지만 응답 예약을
재계산해야 한다. 현재 고정8 + 이전32 + 신규32 + parser16 = 88MiB;
8개 응답에 각5MiB면 총128MiB로 여유가 없다. 4MiB 출력 외 request,
pages/rows capacity 등 실제 할당 증명이 필요하다. 2MiB 예약을 그대로
두는 것은 허용할 수 없다. 미래의 decoded-scan 48MiB 상한까지 고려하면
현재32MiB 구현 기준 계산을 일반 보장으로 사용해서도 안 된다.

대안 B: Volume용 작은 scope 표현과 경량 응답 projection. 기존 byte budget을
지키기 쉽지만 프로토콜/decoder 변경 및 확장 상세표에 필요한 필드 보존을
검토해야 한다. 선택 Page의 별도 조회와 서로 다른 캡처를 한 증거처럼 합치면 안 된다.

두 대안 모두 producer scan cap을 늘리거나 8개의 독립 응답을 합치는 방식은
필요하지 않다. wire 형태의 선택은 사용자에게 떠넘길 제품 질문이 아니며,
합의된 표시 범위와 성능 계약 아래에서 결정할 구현 설계다.

## 렌더링 검토

`web/src/view.tsx:115`의 memo가 age tick과 polling bookkeeping에 따른 map
재구성을 피한다. 새 batch에서는 여전히 로드된 전체 sectors/pages를 순회하고,
섹터별 marks 비교 후 바뀐 DOM을 갱신한다. 조회량 8배가 전체 UI 비용 8배와
동일하다고 단정할 수 없다. `observation-view.tsx:84`의 접힌 상세표는 지연
생성되지만 펼치면 4,096행과 반복 evidence 문자열 비용이 생긴다.

`web/e2e/overlay-density.spec.ts:41`은 12,288 실제 셀, Chromium/Firefox 각각
input-to-visible p95 <=100ms, disabled 대비 탭당 RSS 증가 <=32MiB를 시험한다.
이 기존 시험은 4,096개의 변화하는 runtime 결과 처리 성능을 입증하지 않는다.
새 범위에서는 수신→decode→채택→paint, mode 전환, scroll/resize,
상세표 개폐 및 연속 갱신을 별도 측정해야 한다.

기존 예산의 authoritative 결정은
`../pgbuf-overlay/issues/15-set-overlay-resource-budgets.md`다.
512페이지에서4,096페이지로 늘리는 것은 그 계약의 명시적 설계 개정 사항이다.
선택한 경량 projection에서는 HTTP byte 한도를 유지한다.
기존 producer cap, broker128MiB, browser32MiB, cached HTTP p95 25ms,
refresh p95 250ms, input p95 100ms 등의 gate를 임의로 완화하지 않는다.
4,096페이지 조건에서 이 gate를 통과했다는 주장은 아직 없다.

### 경량 응답 채택 시 UI 검증 범위

현재 `observation-view.tsx:126,135`는 dirty/flushing 필드가 있다고 가정한다.
필드를 생략하기만 하면 mark가 사라지는 것 외에도 tooltip에 `undefined`가
노출될 수 있다. `:86`의 상세표와 `:144`의 legend도 요청 projection에 맞게
구성해야 한다. 응답 종류를 명시적으로 전달하여 Volume은 residency/LRU만
설명하고, 아직 조회하지 않은 상세 사실을 false나 확정 상태로 표시하지 않는다.
Sector는 기존 상세 필드와 해당 legend/table을 유지하는지 별도로 검증한다.

각 응답마다 residency/LRU 값이 실제로 바뀌는 fixture로 4,096개 표시를
검증한다. 같은 capture를 반복 전달하거나 대부분 unknown인 fixture만으로
DOM 갱신 비용을 대표하지 않는다. 동시에 partial fixture도 별도 실행하여
unknown 유지와 렌더링 성능을 혼동하지 않는다.

## 검증할 계약

- 64개의 온전한 섹터, 4,096개의 순서 일치 VPID가 하나의 capture identity로 채택된다.
- 4,095/4,096/4,097 경계와 최대 식별자/최대 evidence 직렬화 byte bound.
- complete, partial, duplicate, unevaluated 및 malformed capture를 구별한다.
  evaluated_count가 4,096이어도 partial omission은 여전히 unknown일 수 있다.
- 1/8/32개 탭, 동시 시작과 시차 시작, 느린 body 소비, 취소 및 429;
  scan coalescing과 요청별 CPU/RSS/직렬화 lock 대기를 구분해 측정한다.
- pause 중 expiry와 restart, 다른 탭의 refresh, resume 이후 scan 시작,
  hidden 복귀, 지연 응답 및 generation 변경. age는 cache 재사용으로 갱신되지 않는다.
- 새로운 범위에서 Chromium/Firefox의 성능·메모리 gate를 각각 재검증한다.

## 제품 결정과 완료 범위

2026-09-11 사용자 “yes”로 두 권장안을 확정했다.

- Q1: 중앙 우선 최대64개 가시 섹터 고정 표시, 초과분 미조회. 시간에 따른 순환 없음.
- Q2: Sector의64페이지는 기존 상세 runtime 정보 유지. Volume만 경량화.

열린 제품 결정은 없다. 요청 단위, 표시 범위, 필드 범위 및 증거 수명 의미를
합의했고 코드 근거와 재현 가능한 크기 모델을 기록했다. 구현, 실제 Rust
직렬화/할당 검증, 1/8/32탭 측정 및 Chromium/Firefox 성능 gate 실행은
후속 구현 작업의 완료 조건이다. 이 설계 완료를 그 gate의 통과로 해석하지 않는다.
