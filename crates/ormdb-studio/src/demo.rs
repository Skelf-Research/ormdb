//! Demo mode with pre-populated movie database.
//!
//! Provides a sample schema and data to help users learn the query language.

use crate::error::{Result, StudioError};
use ormdb_core::catalog::{
    EntityDef, FieldDef, FieldType, RelationDef, ScalarType, SchemaBundle,
};
use ormdb_core::query::encode_entity;
use ormdb_core::storage::{Record, StorageEngine, VersionedKey};
use ormdb_core::Catalog;
use ormdb_proto::Value;

/// Create the movie database schema.
pub fn create_demo_schema() -> SchemaBundle {
    // Genre entity
    let genre = EntityDef::new("Genre", "id")
        .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
        .with_field(FieldDef::new("name", FieldType::scalar(ScalarType::String)));

    // Director entity
    let director = EntityDef::new("Director", "id")
        .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
        .with_field(FieldDef::new("name", FieldType::scalar(ScalarType::String)))
        .with_field(FieldDef::new(
            "birth_year",
            FieldType::scalar(ScalarType::Int32),
        ));

    // Actor entity
    let actor = EntityDef::new("Actor", "id")
        .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
        .with_field(FieldDef::new("name", FieldType::scalar(ScalarType::String)))
        .with_field(FieldDef::new(
            "birth_year",
            FieldType::scalar(ScalarType::Int32),
        ));

    // Movie entity
    let movie = EntityDef::new("Movie", "id")
        .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
        .with_field(FieldDef::new("title", FieldType::scalar(ScalarType::String)))
        .with_field(FieldDef::new("year", FieldType::scalar(ScalarType::Int32)))
        .with_field(FieldDef::new(
            "rating",
            FieldType::scalar(ScalarType::Float32),
        ))
        .with_field(FieldDef::new(
            "genre_id",
            FieldType::scalar(ScalarType::Uuid),
        ))
        .with_field(FieldDef::new(
            "director_id",
            FieldType::scalar(ScalarType::Uuid),
        ));

    // MovieActor junction entity for many-to-many
    let movie_actor = EntityDef::new("MovieActor", "id")
        .with_field(FieldDef::new("id", FieldType::scalar(ScalarType::Uuid)))
        .with_field(FieldDef::new(
            "movie_id",
            FieldType::scalar(ScalarType::Uuid),
        ))
        .with_field(FieldDef::new(
            "actor_id",
            FieldType::scalar(ScalarType::Uuid),
        ));

    // Relations
    // Movie -> Director (many-to-one)
    let movie_director =
        RelationDef::one_to_many("director", "Movie", "director_id", "Director", "id");

    // Movie -> Genre (many-to-one)
    let movie_genre = RelationDef::one_to_many("genre", "Movie", "genre_id", "Genre", "id");

    // Director -> Movies (one-to-many, reverse)
    let director_movies =
        RelationDef::one_to_many("movies", "Director", "id", "Movie", "director_id");

    // Genre -> Movies (one-to-many, reverse)
    let genre_movies = RelationDef::one_to_many("movies", "Genre", "id", "Movie", "genre_id");

    SchemaBundle::new(1)
        .with_entity(genre)
        .with_entity(director)
        .with_entity(actor)
        .with_entity(movie)
        .with_entity(movie_actor)
        .with_relation(movie_director)
        .with_relation(movie_genre)
        .with_relation(director_movies)
        .with_relation(genre_movies)
}

/// Insert demo data into the storage engine.
pub fn insert_demo_data(storage: &StorageEngine, _catalog: &Catalog) -> Result<()> {
    // Generate stable UUIDs for referential integrity
    let genre_ids = generate_uuids(5);
    let director_ids = generate_uuids(5);
    let actor_ids = generate_uuids(10);
    let movie_ids = generate_uuids(10);

    // Insert genres
    let genres = [
        ("Action", genre_ids[0]),
        ("Drama", genre_ids[1]),
        ("Sci-Fi", genre_ids[2]),
        ("Comedy", genre_ids[3]),
        ("Thriller", genre_ids[4]),
    ];

    for (name, id) in &genres {
        insert_entity(
            storage,
            "Genre",
            *id,
            vec![
                ("id".to_string(), Value::Uuid(*id)),
                ("name".to_string(), Value::String(name.to_string())),
            ],
        )?;
    }

    // Insert directors
    let directors = [
        ("Christopher Nolan", 1970, director_ids[0]),
        ("Quentin Tarantino", 1963, director_ids[1]),
        ("Steven Spielberg", 1946, director_ids[2]),
        ("Martin Scorsese", 1942, director_ids[3]),
        ("Denis Villeneuve", 1967, director_ids[4]),
    ];

    for (name, birth_year, id) in &directors {
        insert_entity(
            storage,
            "Director",
            *id,
            vec![
                ("id".to_string(), Value::Uuid(*id)),
                ("name".to_string(), Value::String(name.to_string())),
                ("birth_year".to_string(), Value::Int32(*birth_year)),
            ],
        )?;
    }

    // Insert actors
    let actors = [
        ("Leonardo DiCaprio", 1974, actor_ids[0]),
        ("Tom Hanks", 1956, actor_ids[1]),
        ("Margot Robbie", 1990, actor_ids[2]),
        ("Brad Pitt", 1963, actor_ids[3]),
        ("Scarlett Johansson", 1984, actor_ids[4]),
        ("Robert De Niro", 1943, actor_ids[5]),
        ("Timothée Chalamet", 1995, actor_ids[6]),
        ("Cate Blanchett", 1969, actor_ids[7]),
        ("Samuel L. Jackson", 1948, actor_ids[8]),
        ("Emma Stone", 1988, actor_ids[9]),
    ];

    for (name, birth_year, id) in &actors {
        insert_entity(
            storage,
            "Actor",
            *id,
            vec![
                ("id".to_string(), Value::Uuid(*id)),
                ("name".to_string(), Value::String(name.to_string())),
                ("birth_year".to_string(), Value::Int32(*birth_year)),
            ],
        )?;
    }

    // Insert movies
    // (title, year, rating, director_idx, genre_idx, movie_idx)
    let movies = [
        ("Inception", 2010, 8.8f32, 0, 2, 0),       // Nolan, Sci-Fi
        ("Pulp Fiction", 1994, 8.9f32, 1, 4, 1),    // Tarantino, Thriller
        ("Schindler's List", 1993, 9.0f32, 2, 1, 2), // Spielberg, Drama
        ("Goodfellas", 1990, 8.7f32, 3, 1, 3),      // Scorsese, Drama
        ("Dune", 2021, 8.0f32, 4, 2, 4),            // Villeneuve, Sci-Fi
        ("The Dark Knight", 2008, 9.0f32, 0, 0, 5), // Nolan, Action
        ("Django Unchained", 2012, 8.4f32, 1, 0, 6), // Tarantino, Action
        ("Saving Private Ryan", 1998, 8.6f32, 2, 1, 7), // Spielberg, Drama
        ("The Wolf of Wall Street", 2013, 8.2f32, 3, 3, 8), // Scorsese, Comedy
        ("Arrival", 2016, 7.9f32, 4, 2, 9),         // Villeneuve, Sci-Fi
    ];

    for (title, year, rating, director_idx, genre_idx, movie_idx) in &movies {
        let movie_id = movie_ids[*movie_idx];
        insert_entity(
            storage,
            "Movie",
            movie_id,
            vec![
                ("id".to_string(), Value::Uuid(movie_id)),
                ("title".to_string(), Value::String(title.to_string())),
                ("year".to_string(), Value::Int32(*year)),
                ("rating".to_string(), Value::Float32(*rating)),
                ("director_id".to_string(), Value::Uuid(director_ids[*director_idx])),
                ("genre_id".to_string(), Value::Uuid(genre_ids[*genre_idx])),
            ],
        )?;
    }

    // Insert movie-actor associations (MovieActor junction table)
    // Associate some actors with movies
    let movie_actors = [
        // Inception: DiCaprio
        (movie_ids[0], actor_ids[0]),
        // Pulp Fiction: Jackson
        (movie_ids[1], actor_ids[8]),
        // Schindler's List: (no major actors in our list)
        // Goodfellas: De Niro
        (movie_ids[3], actor_ids[5]),
        // Dune: Chalamet
        (movie_ids[4], actor_ids[6]),
        // The Dark Knight: (no major actors in our list, but let's add Bale via Blanchett for variety)
        (movie_ids[5], actor_ids[7]),
        // Django Unchained: DiCaprio
        (movie_ids[6], actor_ids[0]),
        // Django Unchained: Jackson
        (movie_ids[6], actor_ids[8]),
        // Saving Private Ryan: Hanks
        (movie_ids[7], actor_ids[1]),
        // Wolf of Wall Street: DiCaprio, Robbie
        (movie_ids[8], actor_ids[0]),
        (movie_ids[8], actor_ids[2]),
        // Arrival: (no major actors in our list)
    ];

    for (movie_id, actor_id) in &movie_actors {
        let junction_id = generate_uuid();
        insert_entity(
            storage,
            "MovieActor",
            junction_id,
            vec![
                ("id".to_string(), Value::Uuid(junction_id)),
                ("movie_id".to_string(), Value::Uuid(*movie_id)),
                ("actor_id".to_string(), Value::Uuid(*actor_id)),
            ],
        )?;
    }

    Ok(())
}

/// Insert a single entity into storage.
fn insert_entity(
    storage: &StorageEngine,
    entity_type: &str,
    id: [u8; 16],
    fields: Vec<(String, Value)>,
) -> Result<()> {
    let data =
        encode_entity(&fields).map_err(|e| StudioError::Database(format!("encode error: {}", e)))?;

    let key = VersionedKey::now(id);
    storage
        .put_typed(entity_type, key, Record::new(data))
        .map_err(|e| StudioError::Database(format!("storage error: {}", e)))?;

    Ok(())
}

/// Generate a single UUID.
fn generate_uuid() -> [u8; 16] {
    *uuid::Uuid::new_v4().as_bytes()
}

/// Generate multiple stable UUIDs for demo data.
/// Uses a seed-based approach for reproducibility.
fn generate_uuids(count: usize) -> Vec<[u8; 16]> {
    (0..count).map(|_| generate_uuid()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_demo_schema() {
        let schema = create_demo_schema();

        // Check entities
        assert!(schema.get_entity("Genre").is_some());
        assert!(schema.get_entity("Director").is_some());
        assert!(schema.get_entity("Actor").is_some());
        assert!(schema.get_entity("Movie").is_some());
        assert!(schema.get_entity("MovieActor").is_some());

        // Check Movie fields
        let movie = schema.get_entity("Movie").unwrap();
        assert!(movie.get_field("title").is_some());
        assert!(movie.get_field("year").is_some());
        assert!(movie.get_field("rating").is_some());
        assert!(movie.get_field("director_id").is_some());
        assert!(movie.get_field("genre_id").is_some());

        // Check relations exist
        assert!(schema.get_relation("director").is_some());
        assert!(schema.get_relation("genre").is_some());
    }
}
