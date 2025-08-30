Goals:
Provide a simple, one line bootstrapper. Probably download the shell/docker-compose file from github and start the service.
Simplified way for the users to run this project with all dependencies, without worrying about starting databases/caches
All dependencies for this service is packaged as part of docker compose
The project is containerized with prod configs
Have a way to build and release
A github hook to build the image and publish
DB Migration is handled as part of the startup. We are early in the dev cycle and the schema will change a lot. So there should be proper way to run the migration sctips.
