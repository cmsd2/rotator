import * as cdk from 'aws-cdk-lib';
import * as iam from 'aws-cdk-lib/aws-iam';
import * as codecommit from 'aws-cdk-lib/aws-codecommit';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import * as secretsmanager from 'aws-cdk-lib/aws-secretsmanager';
import { Construct } from 'constructs';

export interface TestStackProps extends cdk.StackProps {
    rotator_lambda: lambda.IFunction,
}

export class TestStack extends cdk.Stack {
  constructor(scope: Construct, id: string, props: TestStackProps) {
    super(scope, id, props);

    let repo = new codecommit.Repository(this, id + '-repo', {
        repositoryName: id + '-repo',
        description: 'repo for testing rotating service-specific credentials',
    });

    let user = new iam.User(this, id + '-user', {
        userName: id,
    });

    let secret = new secretsmanager.Secret(this, id + '-secret', {
        secretName: id,
    });
    cdk.Tags.of(secret).add('rotator:resourceType', 'ServiceSpecificCredential');
    cdk.Tags.of(secret).add('rotator:userName', user.userName);
    cdk.Tags.of(secret).add('rotator:serviceName', 'codecommit.amazonaws.com');

    // addRotationSchedule would add an invoke permission on the rotator that references
    // this stack's secret, creating a cycle. InfraStack already lets Secrets Manager invoke it.
    new secretsmanager.CfnRotationSchedule(this, id + '-rotation', {
        secretId: secret.secretArn,
        rotationLambdaArn: props.rotator_lambda.functionArn,
        rotationRules: {
            automaticallyAfterDays: 7,
        },
    });

    let policy = new iam.ManagedPolicy(this, id + '-user-policy');

    const statements = [
        new iam.PolicyStatement({
            effect: iam.Effect.ALLOW,
            resources: [repo.repositoryArn],
            actions: ['*'],
        }),
        new iam.PolicyStatement({
            effect: iam.Effect.ALLOW,
            resources: [secret.secretArn],
            actions: ['secretsmanager:GetSecretValue'],
            conditions: {
                'ForAnyValue:StringLike': {
                    'secretsmanager:VersionStage' : 'AWSCURRENT',
                }
            }
        })
    ];

    policy.addStatements(...statements);

    policy.attachToUser(user);
  }
}