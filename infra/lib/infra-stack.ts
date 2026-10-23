import * as cdk from 'aws-cdk-lib';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import * as iam from 'aws-cdk-lib/aws-iam';
import * as codedeploy from 'aws-cdk-lib/aws-codedeploy';
import * as cloudwatch from 'aws-cdk-lib/aws-cloudwatch';
import * as cw_actions from 'aws-cdk-lib/aws-cloudwatch-actions';
import * as events from 'aws-cdk-lib/aws-events';
import * as targets from 'aws-cdk-lib/aws-events-targets';
import * as logs from 'aws-cdk-lib/aws-logs';
import * as sns from 'aws-cdk-lib/aws-sns';
import * as subscriptions from 'aws-cdk-lib/aws-sns-subscriptions';
import { Construct } from 'constructs';
import * as path from 'path';
import {lambda_deployment} from './lambda';
import {execSync} from 'child_process';

const ALERT_EMAIL = 'cmsd2@cantab.net';

export interface InfraStackProps extends cdk.StackProps {
  code?: lambda.Code,
}

export class InfraStack extends cdk.Stack {
  rotator_lambda: lambda.IFunction;

  constructor(scope: Construct, id: string, props?: InfraStackProps) {
    super(scope, id, props);

    const hash = process.env.GITHUB_SHA || execSync('git rev-parse --short HEAD').toString('utf8').trim();
    const aliasName = 'prod';

    // A new group, so the auto-created one keeps its history from 2019 on
    const logGroup = new logs.LogGroup(this, id + '-log-group', {
      logGroupName: '/aws/lambda/rotator',
      retention: logs.RetentionDays.ONE_YEAR,
      removalPolicy: cdk.RemovalPolicy.RETAIN,
    });

    let rotator = lambda_deployment(this, 'rotator', {
      aliasName: aliasName,
      code: props?.code ?? lambda.Code.fromAsset(path.join(__dirname, '..', '..', 'target', 'lambda', 'rotator', 'bootstrap.zip')),
      handler: 'unused',
      hash: hash,
      memorySize: 128,
      runtime: lambda.Runtime.PROVIDED_AL2023,
      deploymentConfig: codedeploy.LambdaDeploymentConfig.ALL_AT_ONCE,
      environment: {
        RUST_LOG: 'info'
      },
      logGroup: logGroup,
    });

    const alerts = new sns.Topic(this, id + '-alerts');
    alerts.addSubscription(new subscriptions.EmailSubscription(ALERT_EMAIL));

    // Secrets Manager stops retrying after a few failures, so alert on each one
    new events.Rule(this, id + '-rotation-failed', {
      eventPattern: {
        source: ['aws.secretsmanager'],
        detailType: ['AWS Service Event via CloudTrail'],
        detail: {
          eventName: ['RotationFailed'],
        },
      },
      targets: [new targets.SnsTopic(alerts)],
    });

    const errors = new cloudwatch.Alarm(this, id + '-errors', {
      metric: rotator.lambda.metricErrors({ period: cdk.Duration.hours(1) }),
      threshold: 1,
      evaluationPeriods: 1,
      comparisonOperator: cloudwatch.ComparisonOperator.GREATER_THAN_OR_EQUAL_TO_THRESHOLD,
      treatMissingData: cloudwatch.TreatMissingData.NOT_BREACHING,
    });
    errors.addAlarmAction(new cw_actions.SnsAction(alerts));

    let lambdaPolicy = new iam.ManagedPolicy(this, id + '-lambda-policy');
    
    lambdaPolicy.addStatements(new iam.PolicyStatement({
        effect: iam.Effect.ALLOW,
        actions: [
          'iam:ListServiceSpecificCredentials',
          'iam:*ServiceSpecificCredential',
        ],
        resources: [
          '*'
        ]
      }),
      new iam.PolicyStatement({
        effect: iam.Effect.ALLOW,
        actions: [
          'secretsmanager:DescribeSecret',
          'secretsmanager:GetRandomPassword',
          'secretsmanager:GetSecretValue',
          'secretsmanager:PutSecretValue',
          'secretsmanager:UpdateSecretVersionStage'
        ],
        resources: [
          '*'
        ],
        conditions: {
          StringEquals: {
              'secretsmanager:Resource/AllowRotationLambdaArn': rotator.lambda.functionArn
          }
        }
      }),
    );

    lambdaPolicy.attachToRole(rotator.role);

    rotator.lambda.addPermission('trust-secretsmanager', {
      action: 'lambda:InvokeFunction',
      principal: new iam.ServicePrincipal('secretsmanager.amazonaws.com'),
    });

    this.rotator_lambda = rotator.lambda;
  }
}
