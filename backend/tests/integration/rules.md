1. Integration tests are proper API tests.
2. These tests should talk to the real backend and make all operations via API
3. These teses shuold login/get the token and call the backemd
4. Each tests should have its own data seeding and cleanup
5. The tests should assume the backend is already running and should not start any backends
6. MUST call backend and no creating servers in the test
7. The test should always assert the actual result. Ex, if a get call is expcted to return 200, it should always retun 200. Do not softball and say 403 is also valid and pass the test - it should fail.