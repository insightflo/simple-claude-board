# oh-my-claude-board

> Claude Code 오케스트레이션 TUI 대시보드

터미널에서 Claude Code 에이전트 활동과 태스크 진행 상황을 실시간으로 시각화합니다.

```
+------ Tasks (Tree) ---- v:view ---+---------- Detail ----------+
| ▼ Phase 0 셋업  ████░░ 67%       | P1-R1-T1: 파서 구현        |
|   ├─ [x] P0-T0.1: Cargo 설정     | 상태: InProgress           |
|   └─ [/] P0-T0.2: CI 설정        | 담당: @backend-specialist  |
| ▶ Phase 1 데이터 엔진  33%       | 의존: P0-T0.1              |
| ▼ Phase 2 TUI 코어     0%        +------ Agents --------------+
|   ├─ [ ] P2-S1-T1: 간트          | >> backend-specialist [T1] |
|   └─ [B] P2-S1-T2: 상세          |    -> Edit                 |
|                                   | -- test-specialist         |
+-----------------------------------+----------------------------+
| ✔2 ◀1 ✘1 ⊘4  25%  uptime: 00:05:12  j/k Tab Space v ? q     |
+----------------------------------------------------------------+
```

## 주요 기능

- **실시간 태스크 추적** -- `TASKS.md` 파일 변경을 감지하여 간트 차트를 자동 갱신
- **에이전트 활동 패널** -- 어떤 Claude Code 에이전트가 실행 중인지, 현재 사용 중인 도구, 에러 현황 표시
- **Hook 이벤트 브릿지** -- `event-logger.js` hook이 도구 사용을 JSONL로 기록하여 대시보드에 전달
- **파일 감시** -- `notify` 기반 파일시스템 이벤트 (macOS FSEvents, Linux inotify)
- **듀얼 간트 뷰** -- `▼`/`▶` 접기/펼치기 트리 뷰 + `├─`/`└─` 커넥터, 수평 막대 차트; `v`로 전환
- **Vim 스타일 네비게이션** -- `j`/`k`로 이동, `Tab`으로 패널 전환, `Space`로 접기/펼치기, `?`로 도움말 (한글 IME 지원)
- **~1MB 바이너리** -- LTO + 심볼 스트리핑으로 최적화된 릴리스 빌드

## 사전 요구사항

### Rust (1.75+)

```bash
# macOS / Linux
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# 확인
rustc --version
```

### Node.js (18+, hook 스크립트용)

```bash
# macOS (Homebrew)
brew install node

# Linux (nvm 권장)
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash
nvm install --lts

# 확인
node --version
```

## 설치

```bash
# 소스에서 설치
cargo install --path .

# 또는 로컬 빌드
cargo build --release
# 바이너리 위치: target/release/oh-my-claude-board
```

## CLI 레퍼런스

```
oh-my-claude-board [OPTIONS] [COMMAND]
```

| 옵션 | 기본값 | 설명 |
|---|---|---|
| `--tasks <PATH>` | `./TASKS.md` (대체: `./docs/planning/06-tasks.md`) | TASKS.md 파일 경로 |
| `--hooks <PATH>` | `.claude/hooks` (대체: `~/.claude/hooks`) | Hook JSONL 이벤트 파일 디렉토리 |
| `--events <PATH>` | `~/.claude/dashboard` | 대시보드 JSONL 이벤트 디렉토리 (`event-logger.js`가 기록) |

| 커맨드 | 설명 |
|---|---|
| `watch` (기본) | 파일 감시 + 실시간 TUI 대시보드 표시 |
| `init` | 설정 초기화 (준비 중) |

## 파일 경로

대시보드는 세 곳에서 데이터를 읽습니다:

```
./TASKS.md                          <-- --tasks (프로젝트 태스크 정의)
.claude/hooks/*.jsonl               <-- --hooks (레거시 hook 이벤트 파일)
~/.claude/dashboard/events.jsonl    <-- --events (event-logger.js 출력)
```

- `--tasks`는 단일 파일을 가리킵니다. 상위 디렉토리를 감시합니다.
- `--hooks`와 `--events`는 디렉토리입니다. 내부의 모든 `*.jsonl` 파일을 시작 시 파싱하고, `notify`로 새 쓰기를 감지합니다.
- `--events`의 기본값은 `$HOME/.claude/dashboard`입니다. 첫 도구 사용 시 `event-logger.js`가 자동 생성합니다.
- 세션 ID는 `/tmp/claude-dashboard-session-id`에 저장되어 세션 내 모든 hook 호출에서 공유됩니다.

## 빠른 시작

### 1. Hook 설치

이벤트 로거 hook을 복사하고 Claude Code 설정에 등록합니다:

```bash
# 이벤트 디렉토리 생성
mkdir -p ~/.claude/dashboard

# hook 스크립트 복사 (아직 ~/.claude/hooks/에 없는 경우)
cp hooks/event-logger.js ~/.claude/hooks/event-logger.js
```

`~/.claude/settings.json`의 `PreToolUse`와 `PostToolUse` 양쪽에 추가합니다:

```json
{
  "matcher": "Task|Edit|Write|Read|Bash|Grep|Glob",
  "hooks": [
    {
      "type": "command",
      "command": "node \"${HOME}/.claude/hooks/event-logger.js\"",
      "timeout": 3
    }
  ]
}
```

### 2. 대시보드 실행

```bash
# 기본: ./TASKS.md + ~/.claude/dashboard/events.jsonl 감시
oh-my-claude-board

# 커스텀 경로 지정
oh-my-claude-board watch --tasks ./TASKS.md --hooks .claude/hooks --events ~/.claude/dashboard
```

### 3. Claude Code를 평소처럼 사용

다른 터미널에서 Claude Code를 실행하면, 대시보드에 에이전트 활동이 실시간으로 표시됩니다.

## 작동 원리

```
Claude Code (도구 사용)
       |
       v
 event-logger.js          <-- PreToolUse / PostToolUse hook
       |
       v  (fs.appendFileSync)
 ~/.claude/dashboard/
   events.jsonl            <-- JSONL append-only 로그
       |
       v  (notify 파일 감시)
 oh-my-claude-board        <-- TUI 대시보드
       |
       v
 터미널 (ratatui)
```

**이벤트 흐름:**
1. Claude Code가 도구를 호출 (Edit, Bash, Task 등)
2. `settings.json`이 `event-logger.js`를 Pre/Post hook으로 트리거
3. hook이 `~/.claude/dashboard/events.jsonl`에 JSONL 한 줄 추가
4. 대시보드의 파일 감시자가 변경 감지
5. hook 파서가 새 이벤트를 읽어 `DashboardState` 갱신
6. Agents 패널이 실시간으로 에이전트 상태 렌더링

**JSONL 형식:**
```json
{"event_type":"agent_start","timestamp":"2026-02-08T10:00:00Z","agent_id":"backend-specialist","task_id":"P1-R1-T1","session_id":"sess-abc123","tool_name":"backend-specialist"}
{"event_type":"tool_start","timestamp":"2026-02-08T10:00:01Z","agent_id":"main","task_id":"unknown","session_id":"sess-abc123","tool_name":"Edit"}
```

**TASKS.md 형식** (`nom`으로 파싱):

```markdown
# Phase 0: 셋업

### [x] P0-T0.1: 프로젝트 초기화
- **blocked_by**: (없음)

### [InProgress] P1-R1-T1: 파서 구현
- **blocked_by**: P0-T0.1
```

상태 태그: `[x]` 완료, `[ ]` 대기, `[InProgress]` 또는 `[/]` 진행 중, `[Failed]` 또는 `[!]` 실패, `[Blocked]` 또는 `[B]` 차단됨

## 키바인딩

| 키 | 동작 | 한글 IME |
|---|---|---|
| `j` / `Down` | 아래로 이동 | `ㅓ` |
| `k` / `Up` | 위로 이동 | `ㅏ` |
| `Tab` | 포커스 전환 (태스크 목록 / 상세) | |
| `Space` | Phase 접기/펼치기 | |
| `v` | 뷰 전환 (트리 / 간트 바) | |
| `?` | 도움말 오버레이 토글 | |
| `q` / `Esc` | 종료 | `ㅂ` |

## 레이아웃

```
+------ 55% ------+------ 45% ------+
|                  |     상세 정보    |
|    태스크 목록    |     (70%)       |
|                  +-----------------+
|                  |     에이전트     |
|                  |     (30%)       |
+------------------+-----------------+
|             상태바                  |
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
    tasks_parser.rs    TASKS.md 파서 (nom 조합자)
    hook_parser.rs     JSONL 이벤트 파서 (serde_json)
    watcher.rs         파일 감시자 (notify 6)
    state.rs           통합 DashboardState 모델
  ui/
    layout.rs          화면 분할 계산
    gantt.rs           듀얼 간트 뷰 (트리 + 수평 바)
    detail.rs          태스크 상세 패널
    claude_output.rs   에이전트 활동 패널
    statusbar.rs       하단 상태바
    help.rs            도움말 오버레이 팝업
  analysis/
    rules.rs           에러 패턴 매칭 규칙
    api.rs             선택적 AI 분석 (feature: ai-analysis)
```

## 의존성

| 크레이트 | 버전 | 역할 |
|---|---|---|
| `ratatui` | 0.28 | TUI 렌더링 프레임워크 |
| `crossterm` | 0.28 | 터미널 I/O 백엔드 |
| `tokio` | 1 | 비동기 런타임 (파일 감시자 채널) |
| `clap` | 4 | CLI 인자 파싱 |
| `serde` + `serde_json` | 1 | JSONL 역직렬화 |
| `nom` | 7 | TASKS.md 파서 조합자 |
| `notify` | 6 | 크로스 플랫폼 파일 감시자 (FSEvents/inotify) |
| `chrono` | 0.4 | 타임스탬프 파싱 (serde 지원) |
| `anyhow` + `thiserror` | 1 / 2 | 에러 처리 |
| `tracing` | 0.1 | 구조화된 로깅 |
| `reqwest` | 0.12 | HTTP 클라이언트 (선택적, feature: `ai-analysis`) |

## 개발

```bash
# 테스트 실행 (117 lib + 27 통합 테스트)
cargo test

# 무시된 테스트 포함 실행 (macOS watcher 플래키 테스트)
cargo test --lib -- --include-ignored

# Clippy
cargo clippy -- -D warnings

# 벤치마크
cargo bench
```

## 라이선스

MIT
