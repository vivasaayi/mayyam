package org.poriyiyal.mayyam.userinterface.web.controllers.aws.rds;

import org.poriyiyal.mayyam.userinterface.web.validators.ValidRdsEngine;

import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;

public class CreateInstanceRequest {
    @NotBlank(message = "DB Instance Identifier is required")
    private String dbInstanceIdentifier;

    @NotBlank(message = "DB Instance Class is required")
    private String dbInstanceClass;

    @NotBlank(message = "Engine is required")
    // @ValidRdsEngine
    private String engine;

    @NotNull(message = "Allocated Storage is required")
    @Min(value = 1, message = "Allocated Storage must be at least 1 GB")
    private Integer allocatedStorage;

    // Getters and Setters
    public String getDbInstanceIdentifier() {
        return dbInstanceIdentifier;
    }

    public void setDbInstanceIdentifier(String dbInstanceIdentifier) {
        this.dbInstanceIdentifier = dbInstanceIdentifier;
    }

    public String getDbInstanceClass() {
        return dbInstanceClass;
    }

    public void setDbInstanceClass(String dbInstanceClass) {
        this.dbInstanceClass = dbInstanceClass;
    }

    public String getEngine() {
        return engine;
    }

    public void setEngine(String engine) {
        this.engine = engine;
    }

    public Integer getAllocatedStorage() {
        return allocatedStorage;
    }

    public void setAllocatedStorage(Integer allocatedStorage) {
        this.allocatedStorage = allocatedStorage;
    }
}