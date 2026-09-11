# Volume overlay 설계 결정 트리

확정한 설계와 근거: [spec.md](spec.md). 2026-09-11 사용자 확인 완료.
구현은 아직 시작하지 않았다.

| 결정 | 상태 | 근거 또는 다음 조건 |
| --- | --- | --- |
| Volume은 필요한 residency/LRU만 조회 | 합의 | 사용자 “필요한 부분만 조회”, “LRU list와 resident 여부” |
| Volume/Sector 단위로 scope 표현 | 합의 | 사용자 “volume level 요청 혹은 sector level 요청으로 설계” |
| 동일 공유 캡처, partial unknown, pause/restart 보존 | 요구사항 | 최초 요청 및 기존 ADR-0006 |
| Q1: 가시 섹터가64개를 넘을 때 표시 정책 | 합의 | 사용자 “yes”: 중앙 우선64개 고정·나머지 미조회 |
| Q2: Sector의 상세 필드 범위 | 합의 | 사용자 “yes”: 기존64페이지 상세 유지 |
| 요청 상한·표시 범위 최종 계약 | 확정 | 최대64섹터/4,096페이지; 가시 범위 변경 시 재선택, 시간 순환 없음 |
| Sector 응답 및 legend/table 계약 | 확정 | Sector 상세 유지, Volume은 residency/LRU 전용 표시 |
| 설계 이해 일치 확인 | 완료 | 두 권장안을 최종 질문으로 제시한 뒤 사용자 “yes” 확인 |

열린 제품 결정은 없다. 구현 및 성능 측정은 후속 작업이다.

크기 모델은 [size-model.py](size-model.py)로 재현한다. 현 시점의 크기 계산은
설계 가능성 근거이며 구현의 직렬화·렌더링·다중 탭 성능 통과 증거가 아니다.
