// Copyright (c) 2025 Rajan Panneer Selvam
//
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.mariadb.com/bsl11
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


// Test script to check UUID validation
const { validate, parse } = require('uuid');

// Valid UUID
const validUuid = '123e4567-e89b-12d3-a456-426614174000';
console.log('Valid UUID:', validUuid);
console.log('Is valid:', validate(validUuid));

// Invalid UUID with 'o' at position 2
const invalidUuid = 'tooinvalid-uuid-with-o-at-pos2';
console.log('Invalid UUID:', invalidUuid);
console.log('Is valid:', validate(invalidUuid));
try {
  console.log('Parsed:', parse(invalidUuid));
} catch (e) {
  console.log('Parse error:', e.message);
}

// A UUID with all zeros is valid but special
const zeroUuid = '00000000-0000-0000-0000-000000000001';
console.log('Zero UUID:', zeroUuid);
console.log('Is valid:', validate(zeroUuid));

// This string would have an 'o' at position 2
const problematicId = 'foobarbaz';
console.log('Problematic ID:', problematicId);
console.log('Has "o" at position 2:', problematicId[2] === 'o');
try {
  console.log('Parsed:', parse(problematicId));
} catch (e) {
  console.log('Parse error:', e.message);
}
