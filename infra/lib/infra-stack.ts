import * as cdk from '@aws-cdk/core';
import * as lambda from '@aws-cdk/aws-lambda';
import * as iam from '@aws-cdk/aws-iam';
import * as codedeploy from '@aws-cdk/aws-codedeploy';
import * as path from 'path';
import {execSync} from 'child_process';

export class InfraStack extends cdk.Stack {
  rotator_lambda: lambda.IFunction;

  constructor(scope: cdk.Construct, id: string, props?: cdk.StackProps) {
    super(scope, id, props);

    const hash = process.env.GITHUB_SHA || execSync('git rev-parse --short HEAD').toString('utf8');
    const aliasName = 'prod';

    let role = new iam.Role(this, id + '-lambda-role', {
      assumedBy: new iam.ServicePrincipal('lambda.amazonaws.com')
    });

    let fn = new lambda.Function(this, id + '-function', {
      code: lambda.Code.fromAsset(path.join(__dirname, '..', '..', 'dist', 'rotator.zip')),
      handler: 'unused',
      memorySize: 128,
      runtime: lambda.Runtime.PROVIDED,
      role: role,
      environment: {
        RUST_LOG: 'info'
      }
    });

    role.addManagedPolicy(iam.ManagedPolicy.fromAwsManagedPolicyName("service-role/AWSLambdaBasicExecutionRole"));
    role.addManagedPolicy(iam.ManagedPolicy.fromAwsManagedPolicyName("AWSXrayWriteOnlyAccess"));

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
              'secretsmanager:Resource/AllowRotationLambdaArn': fn.functionArn
          }
        }
      }),
    );

    role.addManagedPolicy(lambdaPolicy);

    fn.addPermission('trust-secretsmanager', {
      action: 'lambda:InvokeFunction',
      principal: new iam.ServicePrincipal('secretsmanager.amazonaws.com'),
    });

    this.rotator_lambda = fn;
  }
}
