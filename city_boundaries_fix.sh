#!/bin/bash
# Run this script in the STC project directory to fix the city_boundaries struct issue

# Set the target project directory
PROJECT_DIR="$HOME/codumeu/STC"

# Function to check if a file exists
file_exists() {
  [ -f "$1" ]
}

# Go to project directory
cd "$PROJECT_DIR" || { echo "Error: Cannot change to project directory"; exit 1; }

echo "Checking for city_boundaries table in schema..."
grep -q "city_boundaries" src/database/schema.rs
if [ $? -ne 0 ]; then
  echo "Error: city_boundaries table not found in schema."
  exit 1
fi

# 1. Fix struct files
echo "Creating correct city_boundaries struct files..."

# Get the current struct content
OLD_STRUCT_FILE="src/structs/generated/city_boundary.rs"
if file_exists "$OLD_STRUCT_FILE"; then
  # Create proper content with correct import
  STRUCT_CONTENT=$(cat "$OLD_STRUCT_FILE" | sed 's/use crate::database::schema::city_boundary;/use crate::database::schema::city_boundaries;/')
  
  # Create correct file
  NEW_STRUCT_FILE="src/structs/generated/city_boundaries.rs"
  echo "$STRUCT_CONTENT" > "$NEW_STRUCT_FILE"
  echo "Created $NEW_STRUCT_FILE"
  
  # Update mod.rs to use correct module name
  MOD_FILE="src/structs/generated/mod.rs"
  if file_exists "$MOD_FILE"; then
    sed -i 's/pub mod city_boundary;/pub mod city_boundaries;/' "$MOD_FILE"
    sed -i 's/pub use city_boundary::/pub use city_boundaries::/' "$MOD_FILE"
    echo "Updated $MOD_FILE"
  fi
  
  # Remove incorrect files
  rm -f "$OLD_STRUCT_FILE"
  echo "Removed $OLD_STRUCT_FILE"
fi

# 2. Fix insertable struct files
OLD_INSERTABLE_FILE="src/structs/generated/insertable/city_boundary.rs"
if file_exists "$OLD_INSERTABLE_FILE"; then
  # Create proper content with correct import
  INSERTABLE_CONTENT=$(cat "$OLD_INSERTABLE_FILE" | sed 's/use crate::database::schema::city_boundary;/use crate::database::schema::city_boundaries;/')
  
  # Create correct file
  NEW_INSERTABLE_FILE="src/structs/generated/insertable/city_boundaries.rs"
  echo "$INSERTABLE_CONTENT" > "$NEW_INSERTABLE_FILE"
  echo "Created $NEW_INSERTABLE_FILE"
  
  # Update insertable/mod.rs to use correct module name
  INSERTABLE_MOD_FILE="src/structs/generated/insertable/mod.rs"
  if file_exists "$INSERTABLE_MOD_FILE"; then
    sed -i 's/pub mod city_boundary;/pub mod city_boundaries;/' "$INSERTABLE_MOD_FILE"
    sed -i 's/pub use city_boundary::/pub use city_boundaries::/' "$INSERTABLE_MOD_FILE"
    echo "Updated $INSERTABLE_MOD_FILE"
  fi
  
  # Remove incorrect files
  rm -f "$OLD_INSERTABLE_FILE"
  echo "Removed $OLD_INSERTABLE_FILE"
fi

# 3. Generate model file if missing
MODELS_DIR="src/models/generated"
MODEL_FILE="$MODELS_DIR/city_boundaries.rs"

if [ ! -d "$MODELS_DIR" ]; then
  mkdir -p "$MODELS_DIR"
  echo "Created $MODELS_DIR directory"
fi

if [ ! -f "$MODEL_FILE" ]; then
  # Create model file with correct content
  cat > "$MODEL_FILE" << 'EOF'
use crate::database::db::establish_connection;
use crate::database::schema::city_boundaries::dsl::{self as city_boundarie_dsl};
use crate::structs::CityBoundary;
use crate::structs::insertable::NewCityBoundary;
use diesel::prelude::*;
use diesel::result::Error;
use diesel::Connection;

impl CityBoundary {
    pub fn get_all() -> Result<Vec<CityBoundary>, &'static str> {
        let mut conn = establish_connection();

        city_boundarie_dsl::city_boundaries
            .order(city_boundarie_dsl::id.asc())
            .load::<CityBoundary>(&mut conn)
            .map_err(|_| "Error retrieving all city_boundaries")
    }

    pub fn get_by_id(id: i32) -> Result<CityBoundary, &'static str> {
        let mut conn = establish_connection();

        match city_boundarie_dsl::city_boundaries.filter(city_boundarie_dsl::id.eq(id)).first::<CityBoundary>(&mut conn) {
            Ok(record) => Ok(record),
            Err(_) => Err("CityBoundary not found"),
        }
    }


    pub fn create(new_record: NewCityBoundary) -> Result<CityBoundary, &'static str> {
        let mut conn = establish_connection();
        
        conn.transaction(|conn| {
            // Insert the new record
            let result = diesel::insert_into(city_boundarie_dsl::city_boundaries)
                .values(&new_record)
                .get_result::<CityBoundary>(conn)
                .map_err(|_| Error::RollbackTransaction)?;
                
            Ok(result)
        })
        .map_err(|e: diesel::result::Error| {
            match e {
                diesel::result::Error::DatabaseError(diesel::result::DatabaseErrorKind::UniqueViolation, _) => {
                    "Record with these values already exists."
                }
                _ => "Error creating new record"
            }
        })
    }

    pub fn update_by_id(id: i32, updates: &NewCityBoundary) -> Result<CityBoundary, &'static str> {
        let mut conn = establish_connection();
        
        conn.transaction(|conn| {
            // Get the latest record data inside the transaction
            let record = city_boundarie_dsl::city_boundaries
                .filter(city_boundarie_dsl::id.eq(id))
                .first::<CityBoundary>(conn)
                .map_err(|_| Error::RollbackTransaction)?;
            
            // Apply updates using NewStruct with AsChangeset
            let updated = diesel::update(city_boundarie_dsl::city_boundaries.filter(city_boundarie_dsl::id.eq(id)))
                .set(updates)
                .get_result::<CityBoundary>(conn)
                .map_err(|_| Error::RollbackTransaction)?;
                
            Ok(updated)
        })
        .map_err(|e: diesel::result::Error| {
            match e {
                diesel::result::Error::DatabaseError(_, _) => "Database error updating record",
                _ => "Error updating record"
            }
        })
    }

    pub fn delete_by_id(id: i32) -> Result<(), &'static str> {
        let mut conn = establish_connection();

        conn.transaction(|conn| {
            // Get the record to confirm it exists
            let _ = city_boundarie_dsl::city_boundaries
                .filter(city_boundarie_dsl::id.eq(id))
                .first::<CityBoundary>(conn)
                .map_err(|_| Error::RollbackTransaction)?;
                
            // Delete the record
            diesel::delete(city_boundarie_dsl::city_boundaries.filter(city_boundarie_dsl::id.eq(id)))
                .execute(conn)
                .map_err(|_| Error::RollbackTransaction)?;
                
            Ok(())
        })
        .map_err(|_: diesel::result::Error| "Error deleting record")
    }

    pub fn count() -> Result<i64, &'static str> {
        let mut conn = establish_connection();
        
        city_boundarie_dsl::city_boundaries
            .count()
            .get_result::<i64>(&mut conn)
            .map_err(|_| "Error counting records")
    }
}
EOF
  echo "Created $MODEL_FILE"

  # Update the models mod.rs file
  MODELS_MOD_FILE="$MODELS_DIR/mod.rs"
  if [ ! -f "$MODELS_MOD_FILE" ]; then
    echo "// Generated model exports" > "$MODELS_MOD_FILE"
  fi
  
  # Add the city_boundaries module to mod.rs if not already there
  grep -q "pub mod city_boundaries;" "$MODELS_MOD_FILE" || {
    echo "pub mod city_boundaries;" >> "$MODELS_MOD_FILE"
    echo "pub use city_boundaries::*;" >> "$MODELS_MOD_FILE"
    echo "Updated $MODELS_MOD_FILE"
  }
fi

echo "Fix completed! The city_boundaries table should now work correctly."