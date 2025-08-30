// JWT Token generation script
const jwt = require('jsonwebtoken');
const fs = require('fs');

// Sample user payload
const userPayload = {
  user_id: 1,
  username: "admin",
  email: "admin@example.com",
  role: "admin",
  sub: "admin", // Required field
  roles: ["admin"], // Required field as an array
  exp: Math.floor(Date.now() / 1000) + (60 * 60 * 24) // 24 hours expiration
};

// Secret key from config.yml
const secretKey = "change_this_to_a_secure_secret_in_production_environment";

// Generate token
const token = jwt.sign(userPayload, secretKey);

// Save token to file
fs.writeFileSync('token.txt', token);

console.log("JWT Token generated and saved to token.txt");
console.log("Token:", token);
