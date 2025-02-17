package org.poriyiyal.mayyam.userinterface.web.validators;

import jakarta.validation.ConstraintValidator;
import jakarta.validation.ConstraintValidatorContext;
import java.util.Arrays;
import java.util.List;

public class RdsEngineValidator implements ConstraintValidator<ValidRdsEngine, String> {

    private final List<String> validEngines = Arrays.asList(
            "mysql", "postgres", "oracle-se2", "oracle-ee", "sqlserver-ee", "sqlserver-se", "sqlserver-ex", "sqlserver-web", "mariadb", "aurora", "aurora-mysql", "aurora-postgresql"
    );

    @Override
    public void initialize(ValidRdsEngine constraintAnnotation) {
    }

    @Override
    public boolean isValid(String value, ConstraintValidatorContext context) {
        return value != null && validEngines.contains(value.toLowerCase());
    }
}