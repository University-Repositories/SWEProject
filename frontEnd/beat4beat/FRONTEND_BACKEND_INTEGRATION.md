# Beat4Beat Frontend Integration Notes

This document describes the current frontend behavior and the fields backend
work will need to supply or receive. The frontend is currently a UI prototype:
the functions below use local React state, static sample data, navigation, or
console output. There are no API calls, authentication requests, persistent
game sessions, or server-driven updates yet.

Source files:

- `src/Home.tsx` — landing page and placeholder lobby/auth actions.
- `src/App.tsx` — game page, local song chain, score, chat, and exit dialog.
- `src/main.tsx` — route setup.

## Routes

| Route | Component | Current behavior |
|---|---|---|
| `/` | `Home` | Landing page. |
| `/game/:gameCode` | `App` | Game page. The route parameter is not currently read by `App`. |
| Any other path | Redirect | Redirects to `/`. |

Quick Queue generates a six-digit number in the browser, logs it, and navigates
to `/game/{gameCode}`. This does not yet create a real matchmaking request.
Forfeit navigates to `/`.

## Frontend functions

### Landing page (`Home.tsx`)

| Function | Parameters | Return/output | Current behavior and backend relevance |
|---|---|---|---|
| `getLandingStats()` | None | `LandingStats` object | Returns hard-coded sample values: `liveBattles: 128`, `globalConnections: 2048`, `syncLatency: "24 ms"`, `longestChain: 37`. Replace with a stats API/query. |
| `Home()` | None | React element | Renders the landing page. Owns modal and form state; uses React Router navigation. |
| `handleSettings()` | None | `void` | Logs that settings were clicked. Connect to user settings UI/API. |
| `handleCreateLobby()` | None | `void` | Logs that lobby creation was clicked. Connect to lobby creation; likely needs current user/session data. |
| `logAuthAction(action)` | `action`: `'Sign in' \| 'Sign up'` | `void` | Logs the action, username, and password. Replace with authentication requests and never log the plaintext password in production. |
| `handleJoinLobby(code)` | `code: string` | `void` | Logs the entered lobby code. Connect to lobby lookup/join and handle invalid/full/expired codes. |
| `handleQuickQueue()` | None | `void` | Generates a random six-digit `gameCode`, logs it, then navigates to `/game/{gameCode}`. Replace random generation/navigation with a matchmaking response from the backend. |

Landing-page local state:

| State | Type | Purpose |
|---|---|---|
| `isSignInOpen` | `boolean` | Controls whether the sign-in/sign-up modal is shown. |
| `lobbyCode` | `string` | Controlled input value submitted to `handleJoinLobby`. |
| `username` | `string` | Controlled authentication username. |
| `password` | `string` | Controlled authentication password. Currently exposed in the debug log; remove that behavior when wiring authentication. |
| `stats` | `LandingStats` | Static values displayed in the four landing-page counters. |

### Game page (`App.tsx`)

| Function | Parameters | Return/output | Current behavior and backend relevance |
|---|---|---|---|
| `createAlbumImage()` | None | `string` data URI | Creates the same inline SVG sample album art for each generated song. Replace `albumImage` with a backend/media URL or asset reference. |
| `createSong(title, owner)` | `title: string`; `owner: 'starting' \| 'player-one' \| 'player-two'` (defaults to `'player-one'`) | `Song` | Creates a sample song. Artist, year, credits, and art are hard-coded; `id` is generated locally using `Date.now() + Math.random()`. Backend song search should return these fields plus stable identifiers and the user/player who made the connection. |
| `createInitialSongs()` | None | `Song[]` | Creates one starting card titled `"Neon Skyline"` with `playerId: "starting"` and an empty player name. Replace with the match's initial/seed song. |
| `App()` | None | React element | Renders the match header, connection form, song chain, chat, and exit dialog. Owns all current match state locally. |
| `showInputFeedback(kind)` | `kind: 'wrong' \| 'valid'` | `void` | Displays red or green input feedback, then clears it after 2.2 seconds. It cancels any earlier feedback timer/frame before showing new feedback. |
| `clearInputFeedback()` | None | `void` | Cancels pending feedback work and removes the current input glow, used when the user edits the input. |
| `submitConnection(event)` | `FormEvent<HTMLFormElement>` | `void` | Trims/logs the entry and clears the input. If the trimmed text is exactly `"wrong"` ignoring case, displays red feedback and makes no card or score change. Any other non-empty value creates a sample song, prepends it to the chain, increments Player 1's score, and shows green feedback. Replace the sample validation/creation with a backend connection-validation request. |
| `submitChat(event)` | `FormEvent<HTMLFormElement>` | `void` | Trims/logs a message and appends it to local chat under the static Player 1 name. Replace local append with a chat message send and server/broadcast update. |

### Data types and constants

```ts
type Song = {
  id: number
  title: string
  artist: string
  year: number
  credits: string
  albumImage: string
  playerId: 'starting' | 'player-one' | 'player-two'
  playerName: string
}

type ChatMessage = {
  id: number
  text: string
}
```

`playerName` is currently the hard-coded string `"Player One"`.
`playerId` is currently `"player-one"`.
`playerLabels` maps the IDs to `"Starting Card"`, `"PLAYER 1"`, and `"PLAYER 2"`.
Use stable backend player/song/message IDs when connecting real sessions.

Game page local state:

| State/ref | Type | Purpose |
|---|---|---|
| `isExitModalOpen` | `boolean` | Controls the forfeit confirmation modal. |
| `connection` | `string` | Current controlled song/connection input. |
| `inputFeedback` | `{ kind: 'wrong' \| 'valid'; id: number } \| null` | Current timed input feedback. The local `id` changes on repeat submissions to restart the animation. |
| `playerOneScore` | `number` | Starts at `0`; increments once per accepted non-empty, non-`"wrong"` entry. |
| `songs` | `Song[]` | Local song chain, newest entry first. |
| `chatInput` | `string` | Current controlled chat input. |
| `messages` | `ChatMessage[]` | Local chat history; message IDs are generated with `Date.now() + Math.random()`. |
| `songListRef` | `HTMLDivElement` ref | Scroll container; smoothly scrolls to the top when the song array changes. |
| `feedbackTimerRef`, `feedbackFrameRef`, `feedbackIdRef` | Timer/frame/number refs | Manage and restart the temporary input glow. |

## DOM IDs and modification hooks

IDs listed as fixed are currently unique and stable in the rendered page.
Song-card IDs include a locally generated song ID and will change each render
session; do not treat them as durable backend identifiers.

| ID | Element/data | Current value/source | Intended use |
|---|---|---|---|
| `home-title` | Landing page heading | Static landing copy | Landing-page heading hook. |
| `lobby-code` | Join-lobby input | React state `lobbyCode` | Lobby code entry. |
| `sign-in-title` | Authentication modal heading | Static copy | Modal/accessibility association. |
| `username` | Authentication input | React state `username` | Sign-in/sign-up identifier. |
| `password` | Authentication input | React state `password` | Credential input; sensitive. |
| `player-one-name` | Match header name | Constant `playerName` | Replace with authenticated/match player display name. |
| `player-one-rating` | Match header rating | Placeholder `[#]` | Replace with Player 1 rating. |
| `player-one-score` | Match header score | React state `playerOneScore` | Update from authoritative match score when backend scoring is connected. |
| `player-two-name` | Opponent name | Placeholder `"Guest Player"` | Replace with opponent display name. |
| `player-two-rating` | Opponent rating | Placeholder `[#]` | Replace with opponent rating. |
| `player-two-score` | Opponent score | Static `0` | Replace with opponent score. |
| `game-timer` | Match timer | Static `100` | Replace with server/match timer updates. |
| `connection-input` | Song/connection input | React state `connection` | Entry submitted for validation. `data-feedback-id` is a transient animation restart token, not a match ID. |
| `song-player-{song.id}` | Song's player badge | `playerLabels[song.playerId]` | Dynamic attribution hook; `{song.id}` is a temporary frontend-generated ID. |
| `song-player-name-{song.id}` | Song's player name | `song.playerName` (empty for starting card) | Dynamic display-name hook; `{song.id}` is a temporary frontend-generated ID. |
| `chat-input` | Chat message input | React state `chatInput` | Message text entered by current player. |
| `exit-title` | Forfeit dialog heading | Static warning | Accessibility association for confirmation dialog. |

The SVG used to draw album art also contains an internal SVG gradient ID `"a"`;
it is unrelated to DOM or backend integration.

## CSS hooks for player-specific appearance

Song cards derive their class from `Song.playerId`:

| `playerId` | Card class | Badge class | Current visual |
|---|---|---|---|
| `starting` | `song-card--starting` | `player-tag--starting` | Neutral gray. |
| `player-one` | `song-card--player-one` | `player-tag--player-one` | Purple card tint; lime badge. |
| `player-two` | `song-card--player-two` | `player-tag--player-two` | Teal card tint and badge. |

To add players, extend the `Song['playerId']` union, `playerLabels` map, and
matching card/badge color rules in `src/App.css`.

## Integration boundary

The current frontend does not define an HTTP/WebSocket API contract. When
adding backend calls, the main operations that need server integration are:

1. Authenticate or register using username/password; return a session/user.
2. Create/join a lobby and quick-queue; return a real match ID and players.
3. Validate a connection against the active song/chain; return acceptance,
   canonical song metadata, and authoritative scores.
4. Send/broadcast chat messages with sender identity and server message IDs.
5. Deliver live stats, player ratings, timer, and score updates.

Prefer stable server-provided identifiers and authoritative scores. Treat DOM
IDs as presentation hooks only; backend code should communicate through an API
or a shared frontend data layer rather than querying or mutating the DOM.
