# simple-claude-board

> Claude Code 오케스트레이션 TUI 대시보드

[English](README.md)

터미널에서 Claude Code 에이전트 활동과 태스크 진행 상황을 실시간으로 시각화합니다.

![simple-claude-board 스크린샷](assets/screenshot.png)

## 주요 기능

- **실시간 태스크 추적** -- `TASKS.md` 파일을 감시하여 저장할 때마다 간트 차트를 자동 갱신
- **에이전트 활동 패널** -- 실행 중인 Claude Code 에이전트, 현재 사용 중인 도구, 에러를 표시
- **풍부한 에이전트 상세** -- 도구 사용 통계, 최근 도구 시퀀스(최근 10개), 세션 ID, 태스크 이름 크로스 참조
- **훅 이벤트 브릿지** -- `event-logger.js` 훅 스크립트가 도구 사용 이벤트를 JSONL로 기록하여 대시보드가 소비
- **에러 분석 & 재시도** -- 12가지 규칙 기반 에러 분류 및 재시도 모달(`r` 키)
- **파일 감시** -- `notify` 크레이트로 파일시스템 이벤트 감지 (macOS: FSEvents, Linux: inotify)
- **이중 간트 뷰** -- 트리 뷰(`▼`/`▶` 접기)와 수평 막대 차트를 `v`로 전환
- **Vim 스타일 탐색** -- `j`/`k`로 이동, `Tab`으로 패널 전환, `Space`로 접기/펼치기, `?`로 도움말
- **한국어 IME 지원** -- 한글 자모(`ㅓ`=j, `ㅏ`=k, `ㅂ`=q)로도 Vim 탐색 가능
- **~1MB 바이너리** -- LTO 및 심볼 제거로 최적화된 릴리스 빌드

## 사전 요구사항

### Rust (1.75+)

```bash
# macOS / Linux
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# Windows — 인스톨러 다운로드 후 실행:
# https://rustup.rs (rustup-init.exe)

# 확인
rustc --version
```

### Node.js (18+, 훅 스크립트용)

```bash
# macOS (Homebrew)
brew install node

# Linux (nvm 권장)
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash
nvm install --lts

# Windows — 인스톨러 다운로드:
# https://nodejs.org

# 확인
node --version
```

## 설치

```bash
# crates.io에서 설치
cargo install simple-claude-board

# 또는 소스에서 설치
git clone https://github.com/insightflo/simple-claude-board.git
cd simple-claude-board
cargo install --path .
```

## CLI 사용법

```
simple-claude-board [OPTIONS] [COMMAND]
```

| 옵션 | 기본값 | 설명 |
|---|---|---|
| `--tasks <PATH>` | `./TASKS.md` (폴백: `./docs/planning/06-tasks.md`) | TASKS.md 파일 경로 |
| `--hooks <PATH>` | `.claude/hooks` (폴백: `~/.claude/hooks`) | 훅 JSONL 이벤트 디렉토리 |
| `--events <PATH>` | `~/.claude/dashboard` | 대시보드 JSONL 이벤트 디렉토리 |

| 명령 | 설명 |
|---|---|
| `watch` (기본) | 파일 감시 및 라이브 TUI 대시보드 표시 |
| `init` | 훅 및 설정 자동 구성 |

## 파일 경로

대시보드는 세 곳에서 데이터를 읽습니다:

```
./TASKS.md                          <-- --tasks (프로젝트 태스크 정의)
.claude/hooks/*.jsonl               <-- --hooks (레거시 훅 이벤트 파일)
~/.claude/dashboard/events.jsonl    <-- --events (event-logger.js 출력)
```

- `--tasks`는 단일 파일을 가리킵니다. 감시기가 부모 디렉토리를 모니터링합니다.
- `--hooks`와 `--events`는 디렉토리입니다. 시작 시 모든 `*.jsonl` 파일을 파싱하고, `notify`로 새 쓰기를 감지합니다.
- `--events`의 기본값은 `$HOME/.claude/dashboard`입니다. 첫 도구 사용 시 `event-logger.js`가 자동 생성합니다.
- 세션 ID는 `/tmp/claude-dashboard-session-id`에 저장되며 세션 내 모든 훅 호출에서 공유됩니다.

## 빠른 시작

```bash
cargo install simple-claude-board
simple-claude-board init    # 훅 & 설정 자동 구성
simple-claude-board         # 대시보드 실행
```

`init` 명령이 자동으로 수행하는 작업:
- `~/.claude/dashboard/` 및 `~/.claude/hooks/` 디렉토리 생성
- `event-logger.js` 훅 스크립트 배포
- `~/.claude/settings.json`에 Pre/PostToolUse 훅 엔트리 패치

그런 다음 다른 터미널을 열고 Claude Code를 정상 사용합니다. 대시보드에 에이전트 활동이 실시간으로 표시됩니다.

### 고급 사용법

```bash
# 커스텀 경로
simple-claude-board watch --tasks ./TASKS.md --hooks .claude/hooks --events ~/.claude/dashboard
```

## 작동 원리

```
Claude Code (도구 사용)
       |
       v
 event-logger.js          <-- PreToolUse / PostToolUse 훅
       |
       v  (fs.appendFileSync)
 ~/.claude/dashboard/
   events.jsonl            <-- JSONL 추가 전용 로그
       |
       v  (notify 파일 감시기)
 simple-claude-board        <-- TUI 대시보드
       |
       v
 Terminal (ratatui)        <-- 렌더링된 UI
```

**이벤트 흐름:**
1. Claude Code가 도구를 호출합니다 (Edit, Bash, Task 등)
2. `settings.json`이 `event-logger.js`를 Pre/Post 훅으로 트리거합니다
3. 훅이 `~/.claude/dashboard/events.jsonl`에 JSONL 한 줄을 추가합니다
4. 대시보드의 파일 감시기가 변경을 감지합니다
5. 훅 파서가 새 이벤트를 읽고 `DashboardState`를 갱신합니다
6. 에이전트 패널이 실시간 상태를 렌더링합니다

**JSONL 형식:**
```json
{"event_type":"agent_start","timestamp":"2026-02-08T10:00:00Z","agent_id":"backend-specialist","task_id":"P1-R1-T1","session_id":"sess-abc123","tool_name":"backend-specialist"}
{"event_type":"tool_start","timestamp":"2026-02-08T10:00:01Z","agent_id":"main","task_id":"unknown","session_id":"sess-abc123","tool_name":"Edit"}
```

**TASKS.md 형식** (`nom`으로 파싱):

```markdown
# Phase 0: Setup

### [x] P0-T0.1: Project init
- **blocked_by**: (none)

### [InProgress] P1-R1-T1: Parser
- **blocked_by**: P0-T0.1
```

상태 태그: `[x]` 완료, `[ ]` 대기, `[InProgress]` 또는 `[/]` 진행중, `[Failed]` 또는 `[!]` 실패, `[Blocked]` 또는 `[B]` 차단

## 키바인딩

| 키 | 동작 | 한글 IME |
|---|---|---|
| `j` / `Down` | 아래로 이동 | `ㅓ` |
| `k` / `Up` | 위로 이동 | `ㅏ` |
| `Tab` | 패널 포커스 전환 (태스크 목록 / 상세) | |
| `Space` | 페이즈 접기/펼치기 | |
| `v` | 뷰 전환 (트리 / 간트 막대) | |
| `r` | 실패 태스크 재시도 | `ㄱ` |
| `?` | 도움말 오버레이 토글 | |
| `q` / `Esc` | 종료 | `ㅂ` |

## 레이아웃

```
+------ 55% ------+------ 45% ------+
|                  |    태스크 상세   |
|   태스크 목록    |     (70%)       |
|                  +-----------------+
|                  |   에이전트 활동  |
|                  |     (30%)       |
+------------------+-----------------+
|             상태 바                |
+------------------------------------+
```

## 아키텍처

```
src/
  main.rs              CLI 진입점 (clap)
  app.rs               앱 상태 + 이벤트 처리
  event.rs             키보드/파일/타이머 이벤트 통합
  lib.rs               크레이트 루트
  data/
    tasks_parser.rs    TASKS.md 파서 (nom 조합기)
    hook_parser.rs     JSONL 이벤트 파서 (serde_json)
    watcher.rs         파일 감시기 (notify 6)
    state.rs           통합 대시보드 상태 모델
    tasks_writer.rs    TASKS.md 상태 쓰기
  ui/
    layout.rs          화면 분할 계산
    gantt.rs           이중 간트 뷰 (트리 + 수평 막대)
    detail.rs          태스크 상세 패널
    claude_output.rs   에이전트 활동 패널
    statusbar.rs       하단 상태 바
    help.rs            도움말 오버레이 팝업
    retry_modal.rs     재시도 확인 모달
  analysis/
    rules.rs           에러 패턴 매칭 규칙
```

## 의존성

| 크레이트 | 버전 | 역할 |
|---|---|---|
| `ratatui` | 0.28 | TUI 렌더링 프레임워크 |
| `crossterm` | 0.28 | 터미널 I/O 백엔드 |
| `tokio` | 1 | 비동기 런타임 (파일 감시기용 채널) |
| `clap` | 4 | CLI 인자 파싱 |
| `serde` + `serde_json` | 1 | JSONL 역직렬화 |
| `nom` | 7 | TASKS.md 파서 조합기 |
| `notify` | 6 | 크로스 플랫폼 파일 감시기 (FSEvents/inotify) |
| `chrono` | 0.4 | 타임스탬프 파싱 (serde 지원) |
| `anyhow` + `thiserror` | 1 / 2 | 에러 처리 |
| `tracing` | 0.1 | 구조화된 로깅 |

## 개발

```bash
# 테스트 실행 (192 lib + 62 integration tests)
cargo test

# 무시된 테스트 포함 실행 (macOS 감시기 불안정 테스트)
cargo test --lib -- --include-ignored

# Clippy
cargo clippy -- -D warnings

# 벤치마크
cargo bench
```

### 성능

| 지표 | 결과 | 목표 |
|---|---|---|
| 1000개 태스크 파싱 | ~745us | <100ms |
| 전체 프레임 렌더 | ~55us | <16ms (60fps) |
| 1000개 훅 이벤트 | ~332us | <100ms |
| 릴리스 바이너리 | ~1.1MB | <10MB |

## 라이선스

MIT
