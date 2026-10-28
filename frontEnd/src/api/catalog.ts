import { request } from './auth'
import type { RecordingDetail } from './generated/RecordingDetail'
import type { RecordingSummary } from './generated/RecordingSummary'
import type { SearchResultsResponse } from './generated/SearchResultsResponse'

// Songs matching a title or artist, best match first. Needs a logged-in session.
export async function searchSongs(query: string, limit = 5): Promise<RecordingSummary[]> {
  const params = new URLSearchParams({ q: query, limit: String(limit) })
  const response = await request(`/api/catalog/search?${params}`)
  const body = (await response.json()) as SearchResultsResponse
  return body.results
}

// One song with all of its credits.
export async function getRecording(id: string): Promise<RecordingDetail> {
  const response = await request(`/api/catalog/recordings/${encodeURIComponent(id)}`)
  return (await response.json()) as RecordingDetail
}
