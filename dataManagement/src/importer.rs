use std::time::Duration;
use musicbrainz_rs::entity::recording::{Recording, RecordingSearchQuery};
use musicbrainz_rs::prelude::*;
use musicbrainz_rs::MusicBrainzClient;
use sqlx::PgPool;

use crate::models::NewSong;
use crate::queries::upsert_song;

/// Fetches songs from MusicBrainz across pop subgenres and inserts them into PostgreSQL.
///
/// Note: MusicBrainz limits search pagination to at most 500 results per query string.
/// To reach 1,000+ songs cleanly without hitting the 500-result cap, this function
/// queries multiple popular subgenres of pop (dance-pop, synth-pop, pop-rock, etc.)
/// and respects the 1 request/second API rate limit.
pub async fn populate_pop_songs(
    pool: &PgPool,
    target_count: usize,
) -> Result<usize, Box<dyn std::error::Error>> {
    let client = MusicBrainzClient::new("Beat4Beat/0.1.0 ( contact@yourproject.org )");

    let pop_tags = [
        "pop",
        "dance-pop",
        "synth-pop",
        "pop-rock",
        "indie-pop",
        "electropop",
        "power-pop",
        "art-pop",
        "k-pop",
        "latin-pop",
        "disco",
        "dream-pop",
        "teen-pop",
        "j-pop",
    ];

    let page_size: u8 = 100;
    let max_offset_per_tag: u16 = 300; // Well below MusicBrainz's 500 limit
    let mut total_inserted = 0;

    println!("Starting import of {} pop songs from MusicBrainz...", target_count);

    'outer: for tag in pop_tags {
        let mut offset: u16 = 0;

        while offset < max_offset_per_tag && total_inserted < target_count {
            let current_limit = ((target_count - total_inserted).min(page_size as usize)) as u8;

            let query_str = RecordingSearchQuery::query_builder()
                .tag(tag)
                .build();

            let mut search_query = Recording::search(query_str);
            search_query.limit(current_limit);
            search_query.offset(offset);

            let results = search_query.execute_with_client(&client)?;

            if results.entities.is_empty() {
                break;
            }

            for recording in results.entities {
                let mut artists = Vec::new();
                if let Some(artist_credits) = recording.artist_credit {
                    for ac in artist_credits {
                        if !artists.contains(&ac.name) {
                            artists.push(ac.name);
                        }
                    }
                }

                let mut albums = Vec::new();
                let mut earliest_year: Option<i32> = None;
                let mut locations = Vec::new();

                if let Some(releases) = recording.releases {
                    for rel in releases {
                        if !albums.contains(&rel.title) {
                            albums.push(rel.title);
                        }
                        if let Some(country) = rel.country {
                            if !locations.contains(&country) {
                                locations.push(country);
                            }
                        }
                        if let Some(date_str) = rel.date {
                            if date_str.0.len() >= 4 {
                                if let Ok(year) = date_str.0[0..4].parse::<i32>() {
                                    earliest_year = match earliest_year {
                                        Some(existing) => Some(existing.min(year)),
                                        None => Some(year),
                                    };
                                }
                            }
                        }
                    }
                }

                let new_song = NewSong {
                    mbid: recording.id,
                    title: recording.title,
                    release_year: earliest_year,
                    duration_ms: recording.length.map(|l| l as i32),
                    artists,
                    albums,
                    credits: Vec::new(),
                    locations,
                };

                if let Ok(_) = upsert_song(pool, &new_song).await {
                    total_inserted += 1;
                }

                if total_inserted >= target_count {
                    println!("Reached target of {} songs!", target_count);
                    break 'outer;
                }
            }

            println!(
                "Tag '{}' [offset {}]: {} / {} pop songs imported",
                tag, offset, total_inserted, target_count
            );

            offset += page_size as u16;

            // Rate limiting pause
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    }

    println!("Completed! Total pop songs imported: {}", total_inserted);
    Ok(total_inserted)
}
