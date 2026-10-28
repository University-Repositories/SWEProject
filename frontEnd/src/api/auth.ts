import type { ApiError } from './generated/ApiError'
import type { User } from './generated/User'

// Thrown when the server answers with a non-2xx status.
export class ApiRequestError extends Error {
  readonly status: number
  readonly body: ApiError | null

  constructor(status: number, body: ApiError | null) {
    super(body?.error.message ?? `Request failed with status ${status}`)
    this.name = 'ApiRequestError'
    this.status = status
    this.body = body
  }
}

async function request(path: string, init: RequestInit = {}): Promise<Response> {
  const response = await fetch(path, init)
  if (!response.ok) {
    let body: ApiError | null = null
    try {
      body = (await response.json()) as ApiError
    } catch {
      body = null
    }
    throw new ApiRequestError(response.status, body)
  }
  return response
}

function postJson(body: unknown): RequestInit {
  return {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  }
}

// Creates the account. It does not log in; call login afterwards.
export async function register(username: string, password: string): Promise<User> {
  const response = await request('/api/auth/register', postJson({ username, password }))
  return (await response.json()) as User
}

// Sets the session cookie. The response has no body; call getCurrentUser for the user.
export async function login(username: string, password: string): Promise<void> {
  await request('/api/auth/login', postJson({ username, password }))
}

// Returns the logged-in user, or null when there is no valid session.
export async function getCurrentUser(): Promise<User | null> {
  try {
    const response = await request('/api/auth/me')
    return (await response.json()) as User
  } catch (error) {
    if (error instanceof ApiRequestError && error.status === 401) return null
    throw error
  }
}

// Clears the session cookie. A missing session counts as already logged out.
export async function logout(): Promise<void> {
  try {
    await request('/api/auth/logout', { method: 'POST' })
  } catch (error) {
    if (error instanceof ApiRequestError && error.status === 401) return
    throw error
  }
}

// Turns an error from the calls above into text for the sign-in modal.
export function describeAuthError(error: unknown): string {
  if (error instanceof ApiRequestError && error.body) {
    const { code, field } = error.body.error
    if (code === 'VALIDATION_FAILED' && field === 'username') {
      return 'Username must be 3 to 20 letters, numbers or underscores.'
    }
    if (code === 'VALIDATION_FAILED' && field === 'password') {
      return 'Password must be 8 to 128 characters.'
    }
    if (code === 'USERNAME_TAKEN') return 'That username is taken.'
    if (code === 'INVALID_CREDENTIALS') return 'Invalid username or password.'
  }
  return 'Something went wrong. Try again.'
}
