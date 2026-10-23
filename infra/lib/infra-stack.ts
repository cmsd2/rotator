import * as cdk from 'aws-cdk-lib';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import * as iam from 'aws-cdk-lib/aws-iam';
import * as codedeploy from 'aws-cdk-lib/aws-codedeploy';
import { Construct } from 'constructs';
import * as path from 'path';
import {lambda_deployment} from './lambda';
import {execSync} from 'child_process';

export interface InfraStackProps extends cdk.StackProps {
  code?: lambda.Code,
}

export class InfraStack extends cdk.Stack {
  rotator_lambda: lambda.IFunction;

  constructor(scope: Construct, id: string, props?: InfraStackProps) {
    super(scope, id, props);

    const hash = process.env.GITHUB_SHA || execSync('git rev-parse --short HEAD').toString('utf8').trim();
    const aliasName = 'prod';

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
      }
    });

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
