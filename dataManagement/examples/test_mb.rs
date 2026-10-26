use musicbrainz_rs::entity::recording::{Recording, RecordingSearchQuery};
use musicbrainz_rs::prelude::*;
use musicbrainz_rs::MusicBrainzClient;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = MusicBrainzClient::new("Beat4Beat/0.1.0 ( contact@yourproject.org )");

    let query_str = RecordingSearchQuery::query_builder()
        .tag("pop")
        .build();

    let mut search_query = Recording::search(query_str);
    search_query.limit(5);
    search_query.offset(0);

    let results = search_query.execute_with_client(&client)?;

    println!("Total matches for tag 'pop': {}", results.count);
    for r in results.entities {
        println!("---------------------------------");
        println!("Title: {}", r.title);
        println!("MBID: {}", r.id);
        if let Some(artists) = r.artist_credit {
            let names: Vec<String> = artists.iter().map(|a| a.name.clone()).collect();
            println!("Artists: {:?}", names);
        }
        if let Some(releases) = r.releases {
            let rel_names: Vec<String> = releases.iter().map(|rel| rel.title.clone()).collect();
            println!("Releases: {:?}", rel_names);
        }
    }

    Ok(())
}
