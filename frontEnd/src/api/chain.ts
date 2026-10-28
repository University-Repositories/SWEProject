import { postJson, request } from './auth'
import type { ChainCheckRequest } from './generated/ChainCheckRequest'
import type { ChainCheckResponse } from './generated/ChainCheckResponse'
import type { ChainStartResponse } from './generated/ChainStartResponse'

// Picks the opening song: the best match for `query`, or a random song when it is empty.
export async function startChain(query = ''): Promise<string> {
  const params = query ? `?${new URLSearchParams({ q: query })}` : ''
  const response = await request(`/api/chain/start${params}`)
  const body = (await response.json()) as ChainStartResponse
  return body.recordingId
}

// Sends the whole chain plus the next move. The server replays it through the game engine.
export async function checkMove(chain: ChainCheckRequest): Promise<ChainCheckResponse> {
  const response = await request('/api/chain/check', postJson(chain))
  return (await response.json()) as ChainCheckResponse
}
