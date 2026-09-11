# 01: Sector 단위 요청으로 기존 상세 관찰 연결

**What to build:** 사용자가 Sector 화면을 열면 sector 하나를 지정하는 요청으로
64페이지의 기존 runtime 상세 정보를 보고, Page 진단과 disk 탐색도 계속 사용할 수 있다.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

## Context

이 티켓은 [구현 명세](../spec.md)의 scope-addressing 경계를 실제 화면까지 연결한다.
기존 producer fixture→HTTP→browser 경계를 확장한다. 독립 schema-only 또는
대규모 prefactor 작업을 선행시키지 않는다. 이후 Volume slice가 같은 검증된
scope discriminator와 decoder-envelope 계약을 사용한다.

## Acceptance criteria

- [ ] Sector 요청이 volume ID와 sector ID 하나로 표현되고 generation의 inspection
  projection으로 검증된다. 실제64페이지 상세 결과가 UI에 표시된다.
- [ ] 기존 dirty/flushing 및 runtime 상세 필드, legend/table/tooltip과 capture
  metadata가 보존된다. 새 payload의 schema/variant와 scope echo를 검증한다.
- [ ] 주소는 sector/page slot으로 정확히 복원된다. 음수/overflow/없는 sector,
  generation mismatch를 거부하고 짧은 마지막 sector의 없는 page를 만들지 않는다.
- [ ] Partial omission/duplicate는 unknown이고 complete 누락만 not-resident다.
  Requested/evaluated와 producer completeness를 독립적으로 표시한다.
- [ ] 기본 pause/resume/expiry/restart 및 지연 응답 거부를 새 요청 경로에서
  회귀 시험한다. 상세 Page와 기존 명시적 VPID 요청의 허용 범위도 유지한다.
- [ ] 기존64KiB request/1MiB response cap 및 runtime admission을 유지한다.
  실제 HTTP의 유효·잘못된 scope와 browser의 상세 표시를 검증한다.
- [ ] 실행 가능한 fixture와 검증 결과, 현재 source baseline을 남긴다. Frontend
  artifacts를 재생성하고 변경 범위의 Rust/frontend 검사와 repository 검증을 통과한다.

## Verification and handoff

기존 HTTP body/scope-boundary, broker lifecycle, browser Sector accessibility와
selected-page 회귀를 우선 사용한다. 완료 시 새 Sector request/response 계약과
외부 관찰 assertion을 기록하여 다음 티켓이 실제 동작을 기준으로 확장하게 한다.
이 티켓의 완료는4,096페이지 표시 또는 성능 측정 통과를 뜻하지 않는다.

## Comments

2026-09-11 — Sector scope는 새로운 addressing과 기존 상세 동작을 함께 검증하는
첫 수직 slice다. 내부 구조 정리가 필요하면 이 완전한 동작을 유지하며 국소적으로 수행한다.

2026-09-11 — 사용자 확인 후 로컬 트래커에 게시. 이 상태는 실행 준비를 뜻하며 구현 완료를 뜻하지 않는다.
