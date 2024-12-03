#!/bin/bash
# Call the endpoint and store the response
response=$(curl -s -w "\nHTTP_STATUS:%{http_code}" -X POST -H "Content-Type: application/json" \
  -d '{"initiator_address":"some_initiator_address"}' http://localhost:3000/trade)

# Extract the HTTP status code and body
http_status=$(echo "$response" | grep -oP '(?<=HTTP_STATUS:)\d+')
body=$(echo "$response" | sed -e 's/HTTP_STATUS:.*//')

# Print the response
echo "Response Body:"
echo "$body"
echo "HTTP Status: $http_status"
