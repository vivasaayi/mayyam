// JWT Token generation script (dev utility)
const jwt = require('jsonwebtoken');
const fs = require('fs');

// Compute issued-at and expiration
const nowSec = Math.floor(Date.now() / 1000);
const expSec = nowSec + 60 * 60 * 24; // 24 hours

// Payload must match backend Claims struct in backend/src/middleware/auth.rs
// Claims { sub: String, username: String, email: Option<String>, roles: Vec<String>, exp: i64, iat: i64 }
const userPayload = {
  sub: "00000000-0000-0000-0000-000000000001", // admin user UUID from database
  username: "admin",
  email: "admin@mayyam.local",
  roles: ["admin", "user"],
  iat: nowSec,
  exp: expSec,
};

// Prefer JWT_SECRET env var; fallback matches backend/config.yml dev default
const secretKey = process.env.JWT_SECRET || "change_this_to_a_secure_secret_in_production_environment";

// Generate token (HS256 by default)
const token = jwt.sign(userPayload, secretKey, { algorithm: 'HS256' });

// Save token to file
fs.writeFileSync('token.txt', token);

console.log("JWT Token generated and saved to token.txt");
console.log("Token:", token);
