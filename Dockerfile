# Use the latest Node.js image to build the React app
FROM node:18 as build

# Set the working directory
WORKDIR /app/reactapp

# Copy the package.json and package-lock.json files
COPY reactapp/package*.json ./

# Install dependencies
RUN npm install --legacy-peer-deps --force

# Copy the React app source code into the Docker image
COPY reactapp /app/reactapp

# Build the React app
RUN npm run build

# Use the OpenJDK 23 JDK image as the base image
FROM maven:3.9-amazoncorretto-23 as compile-image

# Set the working directory
WORKDIR /app

# Copy the built React app to the resources directory of the Java application
COPY --from=build /app/reactapp/build /app/src/main/resources/static

# Copy the application source code into the Docker image
COPY . /app

# Build the Java application (assuming a Maven build system)
RUN mvn clean package

FROM amazoncorretto:23

COPY --from=compile-image /app/target/mayyam-1.0-SNAPSHOT.jar mayyam-1.0-SNAPSHOT.jar

# Specify the command to run the application
CMD ["java", "-jar", "mayyam-1.0-SNAPSHOT.jar"]