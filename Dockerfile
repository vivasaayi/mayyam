# Use the latest Node.js image to build the React app
FROM node:18 as build

# Set the working directory
WORKDIR /app/reactapp

# Copy the package.json and package-lock.json files
COPY reactapp/package*.json ./

# Install dependencies
RUN npm install --legacy-peer-deps --force

# Install crypto polyfill
RUN npm install crypto-browserify --force

# Copy the React app source code into the Docker image
COPY reactapp /app/reactapp

# Add crypto polyfill to the Vite configuration
RUN echo "import { Buffer } from 'buffer';\nimport crypto from 'crypto-browserify';\nwindow.Buffer = Buffer;\nwindow.crypto = crypto;" >> /app/reactapp/src/polyfill.js

# Update the entry point to include the polyfill
RUN sed -i '1s/^/import ".\/polyfill";\n/' /app/reactapp/src/main.js

# Build the React app
RUN npm run build

# Use the OpenJDK 23 JDK image as the base image
FROM openjdk:23-jdk

# Set the working directory
WORKDIR /app

# Copy the built React app to the resources directory of the Java application
COPY --from=build /app/reactapp/build /app/src/main/resources/static

# Copy the application source code into the Docker image
COPY . /app

# Build the Java application (assuming a Maven build system)
RUN ./mvnw clean package

# Specify the command to run the application
CMD ["java", "-jar", "target/your-application.jar"]
