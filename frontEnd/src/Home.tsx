import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import './Home.css'

type LandingStats = {
  liveBattles: number
  globalConnections: number
  syncLatency: string
  longestChain: number
}

function getLandingStats(): LandingStats {
  return {
    liveBattles: 128,
    globalConnections: 2048,
    syncLatency: '24 ms',
    longestChain: 37,
  }
}

function Home() {
  const navigate = useNavigate()
  const [isSignInOpen, setIsSignInOpen] = useState(false)
  const [lobbyCode, setLobbyCode] = useState('')
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const stats = getLandingStats()

  function handleSettings() {
    console.log('[Beat4Beat] Settings button clicked')
  }

  function handleCreateLobby() {
    console.log('[Beat4Beat] Create lobby clicked')
  }

  function logAuthAction(action: 'Sign in' | 'Sign up') {
    console.log(`[Beat4Beat] ${action} requested`, {
      username,
      password,
    })
  }

  function handleJoinLobby(code: string) {
    console.log('[Beat4Beat] Join lobby requested', { code })
  }

  function handleQuickQueue() {
    const gameCode = Math.floor(100000 + Math.random() * 900000)
    console.log('[Beat4Beat] Quick queue started', { gameCode })
    navigate(`/game/${gameCode}`)
  }

  return (
    <div className="home-page">
      <header className="home-header">
        <a className="brand" href="/" aria-label="Beat4Beat home">
          <span>Beat4Beat</span>
        </a>
        <nav className="header-actions" aria-label="Main navigation">
          <button className="settings-button" type="button" onClick={handleSettings}>
            <span aria-hidden="true">⚙</span>
            Settings
          </button>
          <button
            className="sign-in-button"
            type="button"
            onClick={() => setIsSignInOpen(true)}
          >
            Sign in
          </button>
        </nav>
      </header>

      <main className="home-main">
        <section className="home-hero" aria-labelledby="home-title">
          <p className="season-label"><span /> Season 1: The Play Button</p>
          <h1 id="home-title">Connect the Tracks,<br />Rule the Playlist.</h1>
          <p className="hero-description">
            Link songs by author, genre, and other attributes in a high-speed PVP
            battle. One wrong beat, and you chain breaks.
          </p>
          <div className="hero-rule" aria-hidden="true">
            <span />
            <span />
            <span />
          </div>
        </section>

        <section className="lobby-options" aria-label="Choose how to play">
          <button className="mode-card create-card" type="button" onClick={handleCreateLobby}>
            <span className="card-icon create-icon" aria-hidden="true">＋</span>
            <span className="mode-title">CREATE LOBBY</span>
            <span className="mode-subtitle">Host a private session<br />with friends.</span>
            <span className="card-action">SET UP A GAME <span aria-hidden="true">↗</span></span>
          </button>

          <button className="mode-card quick-card" type="button" onClick={handleQuickQueue}>
            <span className="play-icon" aria-hidden="true"><span /></span>
            <span className="mode-title">QUICK QUEUE</span>
            <span className="mode-subtitle">RANKED MATCH MAKING</span>
            <span className="card-action">FIND YOUR MATCH <span aria-hidden="true">↗</span></span>
          </button>

          <form
            className="mode-card join-card"
            onSubmit={(event) => {
              event.preventDefault()
              handleJoinLobby(lobbyCode)
            }}
          >
            <span className="card-icon join-icon" aria-hidden="true">↗</span>
            <span className="mode-title">JOIN LOBBY</span>
            <label className="visually-hidden" htmlFor="lobby-code">Lobby code</label>
            <input
              id="lobby-code"
              className="lobby-code-input"
              type="text"
              placeholder="Enter Code..."
              value={lobbyCode}
              onChange={(event) => setLobbyCode(event.target.value)}
            />
            <button className="join-submit" type="submit">JOIN <span aria-hidden="true">→</span></button>
          </form>
        </section>

        <section className="live-stats" aria-label="Live game statistics">
          <div className="stat-item">
            <span className="stat-value">{stats.liveBattles.toLocaleString()}</span>
            <span className="stat-label">LIVE BATTLES</span>
          </div>
          <div className="stat-item">
            <span className="stat-value">{stats.globalConnections.toLocaleString()}</span>
            <span className="stat-label">GLOBAL CONNECTIONS</span>
          </div>
          <div className="stat-item">
            <span className="stat-value">{stats.syncLatency}</span>
            <span className="stat-label">SYNC LATENCY</span>
          </div>
          <div className="stat-item">
            <span className="stat-value">{stats.longestChain}</span>
            <span className="stat-label">LONGEST CHAIN</span>
          </div>
        </section>
      </main>

      {isSignInOpen && (
        <div
          className="modal-backdrop"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget) setIsSignInOpen(false)
          }}
        >
          <section
            className="sign-in-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="sign-in-title"
          >
            <button
              className="modal-close"
              type="button"
              aria-label="Close sign in dialog"
              onClick={() => setIsSignInOpen(false)}
            >
              ×
            </button>
            <p className="modal-kicker">WELCOME BACK</p>
            <h2 id="sign-in-title">Get in the game.</h2>
            <form className="auth-form" onSubmit={(event) => event.preventDefault()}>
              <label htmlFor="username">Username</label>
              <input
                id="username"
                name="username"
                type="text"
                autoComplete="username"
                value={username}
                onChange={(event) => setUsername(event.target.value)}
              />
              <label htmlFor="password">Password</label>
              <input
                id="password"
                name="password"
                type="password"
                autoComplete="current-password"
                value={password}
                onChange={(event) => setPassword(event.target.value)}
              />
              <div className="auth-actions">
                <button type="button" onClick={() => logAuthAction('Sign in')}>
                  SIGN IN
                </button>
                <button
                  className="signup-action"
                  type="button"
                  onClick={() => logAuthAction('Sign up')}
                >
                  SIGN UP
                </button>
              </div>
            </form>
            <button className="modal-cancel" type="button" onClick={() => setIsSignInOpen(false)}>
              Cancel
            </button>
          </section>
        </div>
      )}
    </div>
  )
}

export default Home
