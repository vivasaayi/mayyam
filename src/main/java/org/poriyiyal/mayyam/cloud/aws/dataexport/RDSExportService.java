package org.poriyiyal.mayyam.cloud.aws.dataexport;

import org.poriyiyal.mayyam.cloud.aws.controlplane.RdsService;
import org.poriyiyal.mayyam.core.services.CsvExportService;
import software.amazon.awssdk.services.rds.model.DBInstance;

import java.io.IOException;
import java.util.List;
import java.util.stream.Collectors;

public class RDSExportService extends BaseExportService<DBInstance> {
    private final RdsService rdsService;

    public RDSExportService(RdsService rdsService, CsvExportService csvExportService) {
        super(csvExportService);
        this.rdsService = rdsService;
    }

    public void exportDBInstancesAsJson(String region, String filePath) throws IOException {
        List<DBInstance> dbInstances = rdsService.listDBInstances(region);
        exportAsJson(dbInstances, filePath);
    }

    public void exportDBInstancesAsCsv(String region, String filePath) throws IOException {
        List<DBInstance> dbInstances = rdsService.listDBInstances(region);
        List<String[]> data = convertToDataFormat(dbInstances);
        String[] headers = {"DB Instance Identifier", "DB Instance Class", "Engine"};
        exportAsCsv(data, headers, filePath);
    }

    public void exportDBInstancesAsCsv(String region, String filePath, char delimiter) throws IOException {
        List<DBInstance> dbInstances = rdsService.listDBInstances(region);
        List<String[]> data = convertToDataFormat(dbInstances);
        String[] headers = {"DB Instance Identifier", "DB Instance Class", "Engine"};
        exportAsCsv(data, headers, filePath, delimiter);
    }

    public void exportDBInstancesAsExcel(String region, String filePath) throws IOException {
        List<DBInstance> dbInstances = rdsService.listDBInstances(region);
        List<String[]> data = convertToDataFormat(dbInstances);
        String[] headers = {"DB Instance Identifier", "DB Instance Class", "Engine"};
        exportAsExcel(data, headers, filePath);
    }

    @Override
    protected List<String[]> convertToDataFormat(List<DBInstance> dbInstances) {
        return dbInstances.stream()
                .map(dbInstance -> new String[]{dbInstance.dbInstanceIdentifier(), dbInstance.dbInstanceClass(), dbInstance.engine()})
                .collect(Collectors.toList());
    }
}