# 02: 같은 캡처의64섹터 residency/LRU를 Volume에 동시 표시

**What to build:** 사용자가 Volume에서 중앙 우선 최대64개 가시 섹터의 residency와
LRU를 동시에 비교하고, 범위를 벗어난 섹터는 미조회로 구분할 수 있다.

**Blocked by:** 01 — Sector 단위 요청으로 기존 상세 관찰 연결. 검증된 view-scope
request/response discriminator와 sector/page-slot addressing을 재사용한다.

**Status:** complete

## Context

[구현 명세](../spec.md)의 핵심 제품 동작이다. 최대4,096개의 결과는 하나의
capture에서 생성되고 하나의 batch로 채택된다.512개 응답8개를 합치지 않는다.
Producer scan 요청은 범위와 무관하게 기존 공유 캡처를 사용한다.

## Acceptance criteria

- [x] Volume 요청은 volume ID와 최대64개 중복 없는 sector ID를 담는다.64개
  이하는 모든 가시 섹터, 초과 시 중앙 거리와 sector ID 우선순위로 선택한다.
- [x] 선택 ID를 canonical 정렬하여 같은 집합의 거리 순서 변화는 batch를
  버리지 않는다. 주기적 순환과 비가시 채움은 없고 scroll/resize는 재선택한다.
- [x] 실제64개 완전한 sector의4,096페이지 결과가 동일 capture identity로
  동시에 표시된다. 초과 가시 섹터는 미조회로 설명하고 expiry 등 상태도 보존한다.
- [x] Volume 응답은 residency state/reason과 LRU zone/kind/index만 포함한다.
  Unknown index, 없음(null), index0, resident-but-LRU-unknown을 구분한다.
- [x] Volume legend/tooltip/상세표도 경량 projection에 맞춘다. Dirty/flushing을
  false로 나타내거나 undefined 문자열을 노출하지 않는다. Sector/Page 상세는 유지한다.
- [x] Scope/epoch/generation/slot 구조를 검증하고 전체 batch를 채택한다.
  기본 complete/partial/duplicate/unevaluated/absent/refused 및 수명 회귀를 포함한다.
- [x] Sector0/1/63/64/65개, 비연속/중복/최대 ID/overflow/짧은 마지막 sector와
  4,095/4,096 유효 페이지를 검증한다.4,097을 요구하는 범위는 허용하지 않는다.
- [x] 최대 폭 resident/LRU와 optional/unknown/null 및 공통 metadata를 포함한
  실제 직렬화가64KiB request/1MiB response 한도를 지킨다. 과대 body/응답은
  bounded하게 실패하며 크기 모델만으로 통과 판정하지 않는다.
- [x] 새 요청의 vectors/output/capacities가 기존 요청별 예약에 포함되는지
  계산·경계 시험하고128MiB 전체 accounting을 유지한다. Cap 확대 없이 구현한다.
- [x] Public HTTP와 browser를 연결한 변화하는4,096-result fixture, 키보드 focus,
  state/LRU 전환·상세표·scroll/resize 시험이 통과한다. 배포 artifacts와 검사 결과를 남긴다.

## Verification and handoff

일반 collection512 제한 및 producer wire v1은 유지한다. Fixture의 visible sector
수, queried page 수, actual rendered cell 수와 capture identity를 별도로 기록한다.
API 상한만 바뀌거나 대부분 unknown인 화면만 통과해서는 완료가 아니다.
성능 적격성의 정식1/8/32탭 및 browser 측정은 후속 티켓에서 수행하되 기본
안전성과 lifecycle correctness는 이 티켓부터 지켜야 한다.

## Comments

2026-09-11 — 합의된 “최대64개 고정 선택”을 구현하며 전체 Volume 조회로 확장하지 않는다.

2026-09-11 — 사용자 확인 후 로컬 트래커에 게시. 이 상태는 실행 준비를 뜻하며 구현 완료를 뜻하지 않는다.


2026-09-11 — 구현 완료. Volume은 가시 섹터 중 중앙 우선 최대64개를 고정 선택하고
canonical ID 순서로 요청한다. 한 캡처의 최대4,096개 residency/LRU 결과를
`volume-residency-lru` 경량 projection으로 함께 채택한다. Sector/Page 상세,
legacy512 및 producer wire v1, 기존 request/response/admission/memory cap은 유지했다.

실제 최대 폭 HTTP 응답554,599bytes, 요청299bytes; 요청별 보수적 allocation
계산1,884,416bytes로 기존2MiB 예약 이내다. Chromium/Firefox 각각 실제12,288셀,
가시120섹터 중64섹터·4,096개 변화하는 결과의 화면 채택과 focus/상세표/scroll/resize를
검증했다. `just verify`: Rust319, Vitest78, browser83통과·기존1skip. 두 축 리뷰
각각 actionable finding0건. 배포 frontend artifact를 재생성했다.

4,095페이지 물리 파일은 ticket01과 동일하게 기존 format validator가
`volume.header.file_length`로 거부한다. 이 입력 계약은 확대하지 않았다.
4,095개 유효 슬롯과 null 뒤 주소 보존은 decoder 시험으로 검증했고, HTTP의
짧은 물리 volume 수용을 완료했다고 주장하지 않는다. 정식 다중 탭/browser/실제
producer 성능 적격성은 후속 티켓에 남는다.

기준 commit은 `6f90d05`다. 계약, raw logs, 실제 브라우저 수치·캡처 및 리뷰는
[검증 및 handoff](../verification/02-volume-scope/README.md)에 기록했다.
