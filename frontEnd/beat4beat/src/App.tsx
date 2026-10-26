import { useEffect, useRef, useState, type FormEvent } from 'react'
import { useNavigate } from 'react-router-dom'
import './App.css'

type Song = {
  id: number
  title: string
  artist: string
  genre: string
  lyrics: string
  albumImage: string
  playerId: 'starting' | 'player-one' | 'player-two'
  playerName: string
}

type ChatMessage = {
  id: number
  text: string
}

const playerName = 'Player One'
const playerId = 'player-one'

const playerLabels: Record<Song['playerId'], string> = {
  starting: 'Starting Card',
  'player-one': 'PLAYER 1',
  'player-two': 'PLAYER 2',
}

function createAlbumImage() {
  const artwork = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 240 240"><defs><linearGradient id="a" x2="1" y2="1"><stop stop-color="#a87cff"/><stop offset="1" stop-color="#c5fa57"/></linearGradient></defs><rect width="240" height="240" fill="#15151f"/><circle cx="120" cy="120" r="91" fill="url(#a)"/><circle cx="120" cy="120" r="31" fill="#111219"/><circle cx="120" cy="120" r="8" fill="#f5f4f7"/><path d="M24 202 90 106l39 48 28-34 60 82z" fill="#08090d" opacity=".55"/></svg>`
  return `data:image/svg+xml,${encodeURIComponent(artwork)}`
}

function createSong(title: string, owner: Song['playerId'] = playerId): Song {
  return {
    id: Date.now() + Math.random(),
    title,
    artist: 'The Midnight Collective',
    genre: 'Synthwave',
    lyrics: 'Running through the city lights\nChasing echoes in the night',
    albumImage: createAlbumImage(),
    playerId: owner,
    playerName: owner === 'starting' ? '' : playerName,
  }
}

function createInitialSongs(): Song[] {
  return [createSong('Neon Skyline', 'starting')]
}

function App() {
  const navigate = useNavigate()
  const [isExitModalOpen, setIsExitModalOpen] = useState(false)
  const [connection, setConnection] = useState('')
  const [inputFeedback, setInputFeedback] = useState<{ kind: 'wrong' | 'valid'; id: number } | null>(null)
  const [playerOneScore, setPlayerOneScore] = useState(0)
  const [songs, setSongs] = useState<Song[]>(createInitialSongs)
  const [chatInput, setChatInput] = useState('')
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const songListRef = useRef<HTMLDivElement>(null)
  const feedbackTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const feedbackFrameRef = useRef<number | null>(null)
  const feedbackIdRef = useRef(0)

  useEffect(() => {
    songListRef.current?.scrollTo({ top: 0, behavior: 'smooth' })
  }, [songs])

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

  function submitConnection(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const title = connection.trim()
    if (!title) return

    console.log('[Beat4Beat] Connection entered', { value: title })
    setConnection('')

    if (title.toLowerCase() === 'wrong') {
      showInputFeedback('wrong')
      return
    }

    showInputFeedback('valid')
    setSongs((currentSongs) => [createSong(title), ...currentSongs])
    setPlayerOneScore((score) => score + 1)
  }

  function submitChat(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const text = chatInput.trim()
    if (!text) return

    console.log('[Beat4Beat] Chat message sent', { sender: playerName, text })
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
          <form className="connection-form" onSubmit={submitConnection}>
            <label className="visually-hidden" htmlFor="connection-input">Make the Connection</label>
            <input
              id="connection-input"
              className={`connection-input${inputFeedback ? ` feedback-${inputFeedback.kind}` : ''}`}
              data-feedback-id={inputFeedback?.id}
              placeholder="Make the Connection"
              value={connection}
              onChange={(event) => {
                setConnection(event.target.value)
                clearInputFeedback()
              }}
            />
            <button className="enter-button" type="submit">ENTER <span aria-hidden="true">→</span></button>
          </form>

          <div className="song-list" ref={songListRef} aria-live="polite">
            {songs.map((song, index) => (
              <div className="song-entry" key={song.id}>
                <article className={`song-card song-card--${song.playerId}`}>
                  <div className="song-credit">
                    <span id={`song-player-${song.id}`} className={`player-tag player-tag--${song.playerId}`}>
                      {playerLabels[song.playerId]}
                    </span>
                    <span
                      id={`song-player-name-${song.id}`}
                      className="song-player-name"
                      aria-label={song.playerName || 'No player assigned'}
                    >
                      {song.playerName}
                    </span>
                  </div>
                  <img className="album-art" src={song.albumImage} alt={`${song.title} album artwork`} />
                  <div className="song-details">
                    <p className="song-label">TRACK CONNECTION</p>
                    <h2>{song.title}</h2>
                    <p className="song-genre">{song.genre}</p>
                    <p className="song-artist"><span>ARTIST</span>{song.artist}</p>
                    <p className="song-lyrics">“{song.lyrics}”</p>
                  </div>
                </article>
                {index < songs.length - 1 && (
                  <div className="song-connector" aria-hidden="true"><span /></div>
                )}
              </div>
            ))}
          </div>
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
