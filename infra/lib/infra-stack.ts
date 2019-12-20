import * as cdk from '@aws-cdk/core';
import * as lambda from '@aws-cdk/aws-lambda';
import * as iam from '@aws-cdk/aws-iam';
import * as codedeploy from '@aws-cdk/aws-codedeploy';
import * as path from 'path';
import {lambda_deployment} from './lambda';
import {execSync} from 'child_process';

export class InfraStack extends cdk.Stack {
  rotator_lambda: lambda.IFunction;

  constructor(scope: cdk.Construct, id: string, props?: cdk.StackProps) {
    super(scope, id, props);

    const hash = process.env.GITHUB_SHA || execSync('git rev-parse --short HEAD').toString('utf8');
    const aliasName = 'prod';

    let rotator = lambda_deployment(this, 'rotator', {
      aliasName: aliasName,
      code: lambda.Code.fromAsset(path.join(__dirname, '..', '..', 'dist', 'rotator.zip')),
      handler: 'unused',
      hash: hash,
      memorySize: 128,
      runtime: lambda.Runtime.PROVIDED,
      deploymentConfig: codedeploy.LambdaDeploymentConfig.ALL_AT_ONCE,
      environment: {
        RUST_LOG: 'info'
      }
    });

    rotator.role.addToPolicy(new iam.PolicyStatement({
      effect: iam.Effect.ALLOW,
      actions: [
        'iam:ListServiceSpecificCredentials',
        'iam:*ServiceSpecificCredential',
      ],
      resources: [
        '*'
      ]
    }));

    rotator.role.addToPolicy(new iam.PolicyStatement({
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
            "secretsmanager:Resource/AllowRotationLambdaArn": rotator.lambda.functionArn
        }
      }
    }))

    rotator.lambda.addPermission('trust-secretsmanager', {
      action: 'lambda:InvokeFunction',
      principal: new iam.ServicePrincipal('secretsmanager.amazonaws.com'),
    });

    this.rotator_lambda = rotator.lambda;
  }
}
