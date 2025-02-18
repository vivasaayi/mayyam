package org.poriyiyal.mayyam.cloud.aws.controlplane;

import org.springframework.stereotype.Service;
import software.amazon.awssdk.services.ec2.Ec2Client;
import software.amazon.awssdk.services.ec2.model.DescribeRegionsRequest;
import software.amazon.awssdk.services.ec2.model.DescribeRegionsResponse;

import java.util.List;
import java.util.stream.Collectors;

@Service
public class AwsRegionService {

    public List<String> listRegions() {
        Ec2Client ec2Client = Ec2Client.builder().build();
        DescribeRegionsRequest request = DescribeRegionsRequest.builder().build();
        DescribeRegionsResponse response = ec2Client.describeRegions(request);
        return response.regions().stream()
                .map(software.amazon.awssdk.services.ec2.model.Region::regionName)
                .collect(Collectors.toList());
    }
}