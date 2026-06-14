import React, { useEffect, useState, useCallback } from "react";
import {
  CCard,
  CCardBody,
  CCardHeader,
  CCol,
  CRow,
  CFormSelect,
  CNav,
  CNavItem,
  CNavLink,
  CAlert,
  CSpinner,
} from "@coreui/react";

import { getAwsAccounts, getInventoryPillarReports } from "../services/api";
import PillarScorecard from "../components/cloud/PillarScorecard";

const SERVICES = [
  { key: "ec2", label: "EC2" },
  { key: "lambda", label: "Lambda" },
  { key: "s3", label: "S3" },
  { key: "rds", label: "RDS" },
  { key: "ebs", label: "EBS" },
  { key: "efs", label: "EFS" },
  { key: "ecs", label: "ECS" },
  { key: "eks", label: "EKS" },
  { key: "dynamodb", label: "DynamoDB" },
  { key: "sqs", label: "SQS" },
  { key: "sns", label: "SNS" },
  { key: "kinesis", label: "Kinesis" },
  { key: "elasticache", label: "ElastiCache" },
  { key: "opensearch", label: "OpenSearch" },
  { key: "vpc", label: "VPC" },
  { key: "iam", label: "IAM" },
  { key: "cloudfront", label: "CloudFront" },
  { key: "elb", label: "ELB" },
  { key: "apigateway", label: "API Gateway" },
  { key: "cloudwatch", label: "CloudWatch" },
  { key: "appsync", label: "AppSync" },
  { key: "glacier", label: "Glacier" },
  { key: "storagegateway", label: "Storage Gateway" },
  { key: "kinesisanalytics", label: "Kinesis Analytics" },
  { key: "subnet", label: "Subnets" },
  { key: "securitygroup", label: "Security Groups" },
  { key: "natgateway", label: "NAT Gateway" },
  { key: "internetgateway", label: "Internet Gateway" },
  { key: "routetable", label: "Route Tables" },
  { key: "networkacl", label: "Network ACLs" },
  { key: "fargate", label: "Fargate" },
  { key: "kms", label: "KMS" },
  { key: "acm", label: "ACM" },
  { key: "cloudtrail", label: "CloudTrail" },
  { key: "config", label: "AWS Config" },
  { key: "eventbridge", label: "EventBridge" },
  { key: "stepfunctions", label: "Step Functions" },
  { key: "apprunner", label: "App Runner" },
  { key: "athena", label: "Athena" },
  { key: "ssm", label: "Systems Manager" },
  { key: "backup", label: "AWS Backup" },
  { key: "batch", label: "Batch" },
  { key: "emr", label: "EMR" },
  { key: "globalaccelerator", label: "Global Accelerator" },
  { key: "glue", label: "Glue" },
  { key: "redshift", label: "Redshift" },
  { key: "waf", label: "WAF" },
  { key: "autoscaling", label: "Auto Scaling" },
  { key: "cloudwatchmetrics", label: "CloudWatch Metrics" },
  { key: "cloudwatchlogs", label: "CloudWatch Logs" },
  { key: "route53", label: "Route 53" },
  { key: "transitgateway", label: "Transit Gateway" },
  { key: "secretsmanager", label: "Secrets Manager" },
  { key: "aurora", label: "Aurora" },
  { key: "msk", label: "MSK" },
  { key: "guardduty", label: "GuardDuty" },
  { key: "securityhub", label: "Security Hub" },
  { key: "inspector", label: "Inspector" },
  { key: "macie", label: "Macie" },
  { key: "organizations", label: "Organizations" },
  { key: "controltower", label: "Control Tower" },
  { key: "servicecatalog", label: "Service Catalog" },
  { key: "trustedadvisor", label: "Trusted Advisor" },
  { key: "computeoptimizer", label: "Compute Optimizer" },
  { key: "health", label: "AWS Health" },
  { key: "resiliencehub", label: "Resilience Hub" },
  { key: "documentdb", label: "DocumentDB" },
  { key: "neptune", label: "Neptune" },
  { key: "memorydb", label: "MemoryDB" },
  { key: "elasticbeanstalk", label: "Elastic Beanstalk" },
  { key: "datasync", label: "DataSync" },
  { key: "fsx", label: "FSx" },
  { key: "timestream", label: "Timestream" },
  { key: "firehose", label: "Firehose" },
  { key: "lakeformation", label: "Lake Formation" },
  { key: "lightsail", label: "Lightsail" },
  { key: "quicksight", label: "QuickSight" },
  { key: "dms", label: "DMS" },
  { key: "mgn", label: "Application Migration" },
  { key: "drs", label: "Elastic Disaster Recovery" },
  { key: "bedrock", label: "Bedrock" },
  { key: "sagemaker", label: "SageMaker AI" },
  { key: "textract", label: "Textract" },
  { key: "comprehend", label: "Comprehend" },
  { key: "amazonmq", label: "Amazon MQ" },
  { key: "privatelink", label: "PrivateLink" },
  { key: "shield", label: "Shield" },
];

// Well-Architected pillar scorecards from deterministic inventory
// evaluators. One tab per supported AWS service.
const PillarScorecards = () => {
  const [accounts, setAccounts] = useState([]);
  const [accountId, setAccountId] = useState("");
  const [service, setService] = useState("ec2");
  const [data, setData] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);

  useEffect(() => {
    getAwsAccounts()
      .then((resp) => {
        const list = Array.isArray(resp) ? resp : resp?.accounts || [];
        setAccounts(list);
        if (list.length > 0) {
          setAccountId(list[0].account_id);
        }
      })
      .catch(() => setError("Failed to load AWS accounts"));
  }, []);

  const load = useCallback(async (svc, acct) => {
    if (!acct) return;
    setLoading(true);
    setError(null);
    try {
      const resp = await getInventoryPillarReports(svc, acct);
      setData(resp);
    } catch (e) {
      setData(null);
      setError(`Failed to load ${svc} pillar reports`);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load(service, accountId);
  }, [service, accountId, load]);

  return (
    <CRow>
      <CCol>
        <CCard className="mb-3">
          <CCardHeader>AWS Pillar Scorecards</CCardHeader>
          <CCardBody>
            <CRow className="mb-3">
              <CCol sm={4}>
                <CFormSelect
                  value={accountId}
                  onChange={(e) => setAccountId(e.target.value)}
                  aria-label="AWS account"
                >
                  {accounts.length === 0 && (
                    <option value="">No AWS accounts configured</option>
                  )}
                  {accounts.map((a) => (
                    <option key={a.account_id} value={a.account_id}>
                      {a.account_name || a.account_id}
                    </option>
                  ))}
                </CFormSelect>
              </CCol>
            </CRow>
            <CNav variant="tabs" className="mb-3">
              {SERVICES.map((s) => (
                <CNavItem key={s.key}>
                  <CNavLink
                    href="#"
                    active={service === s.key}
                    onClick={(e) => {
                      e.preventDefault();
                      setService(s.key);
                    }}
                  >
                    {s.label}
                  </CNavLink>
                </CNavItem>
              ))}
            </CNav>
            {error && <CAlert color="danger">{error}</CAlert>}
            {loading && <CSpinner size="sm" />}
            {!loading && data && <PillarScorecard data={data} />}
          </CCardBody>
        </CCard>
      </CCol>
    </CRow>
  );
};

export default PillarScorecards;
