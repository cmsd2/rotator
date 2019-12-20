import * as cdk from '@aws-cdk/core';
import * as iam from '@aws-cdk/aws-iam';
import * as codecommit from '@aws-cdk/aws-codecommit';
import * as lambda from '@aws-cdk/aws-lambda';
import * as secretsmanager from '@aws-cdk/aws-secretsmanager';

export interface TestStackProps extends cdk.StackProps {
    rotator_lambda: lambda.IFunction,
}

export class TestStack extends cdk.Stack {
  constructor(scope: cdk.Construct, id: string, props: TestStackProps) {
    super(scope, id, props);

    let repo = new codecommit.Repository(this, id + '-repo', {
        repositoryName: id + '-repo',
        description: 'repo for testing rotating service-specific credentials',
    });

    let policy = new iam.ManagedPolicy(this, id + 'user-policy');

    policy.addStatements(new iam.PolicyStatement({
        effect: iam.Effect.ALLOW,
        resources: [repo.repositoryArn],
        actions: ['*'],
    }));

    let user = new iam.User(this, id + '-user', {
        userName: id,
        permissionsBoundary: policy,
    });

    let secret = new secretsmanager.Secret(this, id + '-secret', {
        secretName: id,
    });
    cdk.Tag.add(secret, 'rotator:resourceType', 'ServiceSpecificCredential');
    cdk.Tag.add(secret, 'rotator:userName', user.userName);
    cdk.Tag.add(secret, 'rotator:serviceName', 'codecommit.amazonaws.com');

    secret.addRotationSchedule(id + '-rotation', {
        rotationLambda: props.rotator_lambda,
        automaticallyAfter: cdk.Duration.days(7),
    });
  }
}