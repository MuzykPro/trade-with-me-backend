#!/bin/bash

# Exit immediately if a command exits with a non-zero status
set -e

# ----------------------------
# Configuration - Replace these with your actual database credentials
DB_USERNAME="trade_user"
DB_PASSWORD="password_trade_123"
DB_HOST="localhost"
DB_PORT="5432"
DB_NAME="trade_with_me"
# ----------------------------

# Construct the DATABASE_URL environment variable
export DATABASE_URL="postgres://${DB_USERNAME}:${DB_PASSWORD}@${DB_HOST}:${DB_PORT}/${DB_NAME}"

# Optional: Print the DATABASE_URL for verification (comment out in production)
echo "DATABASE_URL is set to: $DATABASE_URL"

# Run Diesel migrations
diesel migration run

# Check the exit status of the diesel command
if [ $? -eq 0 ]; then
  echo "Migrations ran successfully."
else
  echo "An error occurred while running migrations."
  exit 1
fi

diesel print-schema > src/schema.rs