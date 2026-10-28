import { useEffect, useRef, useState, type FormEvent } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { ApiRequestError, getCurrentUser } from './api/auth'
import { getRecording, searchSongs } from './api/catalog'
import { checkMove, startChain } from './api/chain'
import type { ChainMove } from './api/generated/ChainMove'
import type { MoveRejectionCode } from './api/generated/MoveRejectionCode'
import type { RecordingDetail } from './api/generated/RecordingDetail'
import './App.css'

// Matches MAX_CREDIT_USES in the game engine.
const MAX_CREDIT_USES = 3

type PlayerId = 'starting' | 'player-one' | 'player-two'

type ChainCard = {
  key: string
  song: RecordingDetail
  playerId: PlayerId
  playerName: string
  linkedBy: { name: string; usesLeft: number } | null
}

// One credit entity on a song, with every role it has there.
type CreditGroup = {
  creditId: string
  name: string
  roles: string[]
}

type ChatMessage = {
  id: number
  text: string
}

type GameStatus = 'loading' | 'signed-out' | 'ready' | 'error'

const playerLabels: Record<PlayerId, string> = {
  starting: 'Starting Card',
  'player-one': 'PLAYER 1',
  'player-two': 'PLAYER 2',
}

const rejectionText: Record<MoveRejectionCode, string> = {
  SONG_ALREADY_PLAYED: 'That song is already in the chain.',
  CREDIT_NOT_ON_PREVIOUS_SONG: 'That credit is not on the previous song.',
  CREDIT_NOT_ON_SUBMITTED_SONG: 'That credit is not on the song you entered.',
  CREDIT_EXHAUSTED: `That credit has been used ${MAX_CREDIT_USES} times already.`,
}

function createAlbumImage() {
  const artwork = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 240 240"><defs><linearGradient id="a" x2="1" y2="1"><stop stop-color="#a87cff"/><stop offset="1" stop-color="#c5fa57"/></linearGradient></defs><rect width="240" height="240" fill="#15151f"/><circle cx="120" cy="120" r="91" fill="url(#a)"/><circle cx="120" cy="120" r="31" fill="#111219"/><circle cx="120" cy="120" r="8" fill="#f5f4f7"/><path d="M24 202 90 106l39 48 28-34 60 82z" fill="#08090d" opacity=".55"/></svg>`
  return `data:image/svg+xml,${encodeURIComponent(artwork)}`
}

const albumImage = createAlbumImage()

function groupCredits(song: RecordingDetail): CreditGroup[] {
  const groups = new Map<string, CreditGroup>()
  for (const credit of song.credits) {
    const group = groups.get(credit.creditId)
    if (group) {
      if (!group.roles.includes(credit.role)) group.roles.push(credit.role)
    } else {
      groups.set(credit.creditId, { creditId: credit.creditId, name: credit.name, roles: [credit.role] })
    }
  }
  return [...groups.values()]
}

function describeCredits(song: RecordingDetail): string {
  return groupCredits(song)
    .map((group) => `${group.name} (${group.roles.join(', ')})`)
    .join(' · ')
}

function App() {
  const navigate = useNavigate()
  const [searchParams] = useSearchParams()
  const startQuery = searchParams.get('start') ?? ''
  const [isExitModalOpen, setIsExitModalOpen] = useState(false)
  const [connection, setConnection] = useState('')
  const [inputFeedback, setInputFeedback] = useState<{ kind: 'wrong' | 'valid'; id: number } | null>(null)
  const [playerOneScore, setPlayerOneScore] = useState(0)
  const [playerName, setPlayerName] = useState('Player One')
  const [status, setStatus] = useState<GameStatus>('loading')
  const [cards, setCards] = useState<ChainCard[]>([])
  const [openingId, setOpeningId] = useState<string | null>(null)
  const [moves, setMoves] = useState<ChainMove[]>([])
  const [creditUses, setCreditUses] = useState<Record<string, number>>({})
  const [candidate, setCandidate] = useState<RecordingDetail | null>(null)
  const [notice, setNotice] = useState('')
  const [isBusy, setIsBusy] = useState(false)
  const [chatInput, setChatInput] = useState('')
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const songListRef = useRef<HTMLDivElement>(null)
  const feedbackTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const feedbackFrameRef = useRef<number | null>(null)
  const feedbackIdRef = useRef(0)

  const currentSong = cards.length > 0 ? cards[0].song : null
  const currentCreditIds = new Set(currentSong?.credits.map((credit) => credit.creditId) ?? [])

  // Check the session, then pick the opening song. ?start=<title> picks it by search.
  useEffect(() => {
    let cancelled = false
    async function beginGame() {
      const user = await getCurrentUser()
      if (cancelled) return
      if (!user) {
        setStatus('signed-out')
        return
      }
      setPlayerName(user.username)
      const recordingId = await startChain(startQuery)
      const opening = await getRecording(recordingId)
      if (cancelled) return
      setOpeningId(opening.id)
      setCards([{ key: opening.id, song: opening, playerId: 'starting', playerName: '', linkedBy: null }])
      setMoves([])
      setCreditUses({})
      setCandidate(null)
      setPlayerOneScore(0)
      setNotice('Enter a song that shares a credit with the starting card.')
      setStatus('ready')
    }
    beginGame().catch(() => {
      if (!cancelled) setStatus('error')
    })
    return () => {
      cancelled = true
    }
  }, [startQuery])

  useEffect(() => {
    songListRef.current?.scrollTo({ top: 0, behavior: 'smooth' })
  }, [cards])

  useEffect(() => () => {
    if (feedbackTimerRef.current) clearTimeout(feedbackTimerRef.current)
    if (feedbackFrameRef.current !== null) cancelAnimationFrame(feedbackFrameRef.current)
  }, [])

  function showInputFeedback(kind: 'wrong' | 'valid') {
    if (feedbackTimerRef.current) clearTimeout(feedbackTimerRef.current)
    if (feedbackFrameRef.current !== null) cancelAnimationFrame(feedbackFrameRef.current)

    setInputFeedback(null)
    feedbackFrameRef.current = requestAnimationFrame(() => {
      setInputFeedback({ kind, id: ++feedbackIdRef.current })
      feedbackFrameRef.current = null
      feedbackTimerRef.current = setTimeout(() => setInputFeedback(null), 2200)
    })
  }

  function clearInputFeedback() {
    if (feedbackTimerRef.current) clearTimeout(feedbackTimerRef.current)
    if (feedbackFrameRef.current !== null) cancelAnimationFrame(feedbackFrameRef.current)
    feedbackTimerRef.current = null
    feedbackFrameRef.current = null
    setInputFeedback(null)
  }

  function handleRequestError(error: unknown) {
    if (error instanceof ApiRequestError && error.status === 401) {
      setStatus('signed-out')
      return
    }
    setNotice('Something went wrong. Try again.')
    showInputFeedback('wrong')
  }

  // Step 1: find the song the player typed and show its credits.
  async function findSong(title: string) {
    setIsBusy(true)
    try {
      const results = await searchSongs(title, 1)
      if (results.length === 0) {
        setCandidate(null)
        setNotice(`No song found for “${title}”.`)
        showInputFeedback('wrong')
        return
      }
      const song = await getRecording(results[0].id)
      setCandidate(song)
      setNotice(`Pick the credit that links ${song.title} to ${currentSong?.title ?? 'the last song'}.`)
    } catch (error) {
      handleRequestError(error)
    } finally {
      setIsBusy(false)
    }
  }

  // Step 2: send the chain and the chosen credit to the server, which runs the game engine.
  async function chooseCredit(group: CreditGroup) {
    if (!candidate || !openingId || isBusy) return
    const next: ChainMove = { recordingId: candidate.id, creditId: group.creditId }
    setIsBusy(true)
    try {
      const result = await checkMove({ openingId, moves, next })
      if (result.accepted) {
        const usesAfter = result.usesAfter ?? 1
        const usesLeft = MAX_CREDIT_USES - usesAfter
        const linkedSong = candidate
        setMoves((current) => [...current, next])
        setCreditUses((current) => ({ ...current, [group.creditId]: usesAfter }))
        setCards((current) => [
          {
            key: `${linkedSong.id}-${current.length}`,
            song: linkedSong,
            playerId: 'player-one',
            playerName,
            linkedBy: { name: group.name, usesLeft },
          },
          ...current,
        ])
        setPlayerOneScore((score) => score + 1)
        setCandidate(null)
        setNotice(`Linked by ${group.name}. ${usesLeft} ${usesLeft === 1 ? 'use' : 'uses'} left.`)
        showInputFeedback('valid')
      } else {
        setNotice(result.rejection ? rejectionText[result.rejection] : 'That link is not allowed.')
        showInputFeedback('wrong')
        if (result.rejection === 'SONG_ALREADY_PLAYED') setCandidate(null)
      }
    } catch (error) {
      handleRequestError(error)
    } finally {
      setIsBusy(false)
    }
  }

  function submitConnection(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const title = connection.trim()
    if (!title || isBusy || status !== 'ready') return
    setConnection('')
    void findSong(title)
  }

  function submitChat(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const text = chatInput.trim()
    if (!text) return

    setMessages((currentMessages) => [
      ...currentMessages,
      { id: Date.now() + Math.random(), text },
    ])
    setChatInput('')
  }

  return (
    <main className="game-page">
      <header className="game-title">
        <h1>Beat4Beat</h1>
      </header>
      <header className="game-header game-panel">
        <button className="exit-button" type="button" onClick={() => setIsExitModalOpen(true)}>
          ← <span>Exit Game</span>
        </button>

        <div className="match-players" aria-label="Match players and scores">
          <div className="match-player">
            <div className="player-name-line">
              <span id="player-one-name">{playerName}</span>
              <span id="player-one-rating" className="player-rating">[#]</span>
            </div>
            <span id="player-one-score" className="player-score">{playerOneScore}</span>
          </div>
          <span className="versus">VS</span>
          <div className="match-player opponent">
            <div className="player-name-line">
              <span id="player-two-name">Guest Player</span>
              <span id="player-two-rating" className="player-rating">[#]</span>
            </div>
            <span id="player-two-score" className="player-score">0</span>
          </div>
        </div>

        <div className="game-timer" id="game-timer" aria-label="Time remaining">100</div>
      </header>

      <div className="game-layout">
        <section className="game-card game-panel" aria-label="Music connection game">
          {status === 'loading' && <p className="game-status">Starting a game…</p>}

          {status === 'signed-out' && (
            <div className="game-status">
              <p>Sign in on the home page to play.</p>
              <button className="enter-button" type="button" onClick={() => navigate('/')}>
                GO TO SIGN IN
              </button>
            </div>
          )}

          {status === 'error' && (
            <div className="game-status">
              <p>Couldn't start a game. Check that the server is running and songs are imported.</p>
              <button className="enter-button" type="button" onClick={() => window.location.reload()}>
                TRY AGAIN
              </button>
            </div>
          )}

          {status === 'ready' && (
            <>
              <form className="connection-form" onSubmit={submitConnection}>
                <label className="visually-hidden" htmlFor="connection-input">Make the Connection</label>
                <input
                  id="connection-input"
                  className={`connection-input${inputFeedback ? ` feedback-${inputFeedback.kind}` : ''}`}
                  data-feedback-id={inputFeedback?.id}
                  placeholder="Make the Connection"
                  value={connection}
                  disabled={isBusy}
                  onChange={(event) => {
                    setConnection(event.target.value)
                    clearInputFeedback()
                  }}
                />
                <button className="enter-button" type="submit" disabled={isBusy}>
                  ENTER <span aria-hidden="true">→</span>
                </button>
              </form>

              {notice && <p className="chain-notice" role="status">{notice}</p>}

              {candidate && (
                <div className="candidate-panel">
                  <p className="candidate-title">
                    <span>NEXT SONG</span>
                    {candidate.title} · {candidate.artistName}
                  </p>
                  <div className="credit-options">
                    {groupCredits(candidate).map((group) => {
                      const shared = currentCreditIds.has(group.creditId)
                      const usesLeft = MAX_CREDIT_USES - (creditUses[group.creditId] ?? 0)
                      return (
                        <button
                          key={group.creditId}
                          type="button"
                          className={`credit-option${shared ? ' credit-option--shared' : ''}`}
                          disabled={isBusy}
                          onClick={() => void chooseCredit(group)}
                        >
                          <span className="credit-option-name">{group.name}</span>
                          <span className="credit-option-meta">
                            {group.roles.join(', ')}
                            {shared ? ` · on both songs · ${usesLeft} left` : ''}
                          </span>
                        </button>
                      )
                    })}
                  </div>
                  <button className="candidate-cancel" type="button" onClick={() => setCandidate(null)}>
                    Pick a different song
                  </button>
                </div>
              )}

              <div className="song-list" ref={songListRef} aria-live="polite">
                {cards.map((card, index) => (
                  <div className="song-entry" key={card.key}>
                    <article className={`song-card song-card--${card.playerId}`}>
                      <div className="song-credit">
                        <span id={`song-player-${card.key}`} className={`player-tag player-tag--${card.playerId}`}>
                          {playerLabels[card.playerId]}
                        </span>
                        <span
                          id={`song-player-name-${card.key}`}
                          className="song-player-name"
                          aria-label={card.playerName || 'No player assigned'}
                        >
                          {card.playerName}
                        </span>
                      </div>
                      <img className="album-art" src={albumImage} alt={`${card.song.title} album artwork`} />
                      <div className="song-details">
                        <p className="song-label">
                          {card.linkedBy
                            ? `LINKED BY ${card.linkedBy.name.toUpperCase()} · ${card.linkedBy.usesLeft} LEFT`
                            : 'TRACK CONNECTION'}
                        </p>
                        <h2>{card.song.title}</h2>
                        <p className="song-year"><span>YEAR</span>{card.song.year ?? 'Unknown'}</p>
                        <p className="song-artist"><span>ARTIST</span>{card.song.artistName}</p>
                        <p className="song-credits"><span>CREDITS</span>{describeCredits(card.song)}</p>
                      </div>
                    </article>
                    {index < cards.length - 1 && (
                      <div className="song-connector" aria-hidden="true"><span /></div>
                    )}
                  </div>
                ))}
              </div>
            </>
          )}
        </section>
        <aside className="chat-card game-panel" aria-label="Game chat">
          <h2 className="chat-title">Chat <span>LIVE</span></h2>
          <div className="chat-messages" aria-live="polite">
            {messages.length === 0 ? (
              <p className="chat-empty">Messages will appear here.</p>
            ) : (
              messages.map((message) => (
                <article className="chat-message" key={message.id}>
                  <strong>{playerName}</strong>
                  <p>{message.text}</p>
                </article>
              ))
            )}
          </div>
          <form className="chat-form" onSubmit={submitChat}>
            <label className="visually-hidden" htmlFor="chat-input">Send a chat message</label>
            <input
              id="chat-input"
              placeholder="Type a message..."
              value={chatInput}
              onChange={(event) => setChatInput(event.target.value)}
            />
            <button type="submit" aria-label="Send message">↑</button>
          </form>
        </aside>
      </div>

      {isExitModalOpen && (
        <div className="exit-backdrop">
          <section className="exit-modal game-panel" role="alertdialog" aria-modal="true" aria-labelledby="exit-title">
            <p className="exit-kicker">LEAVE MATCH</p>
            <h2 id="exit-title">Are you sure your want to exit the game?</h2>
            <p>This automatically forfeits you from the game.</p>
            <div className="exit-actions">
              <button className="forfeit-button" type="button" onClick={() => navigate('/')}>FORFEIT</button>
              <button className="cancel-button" type="button" onClick={() => setIsExitModalOpen(false)}>CANCEL</button>
            </div>
          </section>
        </div>
      )}
    </main>
  )
}

export default App
