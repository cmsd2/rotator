import * as cdk from 'aws-cdk-lib';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import { Match, Template } from 'aws-cdk-lib/assertions';
import { InfraStack } from '../lib/infra-stack';
import { GithubActionsRoleStack } from '../lib/github-actions-role-stack';

test('rotator runs on provided.al2023 with automatic runtime updates', () => {
    const app = new cdk.App();
    const stack = new InfraStack(app, 'MyTestStack', {
      code: lambda.Code.fromAsset(__dirname),
    });

    Template.fromStack(stack).hasResourceProperties('AWS::Lambda::Function', {
      Runtime: 'provided.al2023',
      RuntimeManagementConfig: { UpdateRuntimeOn: 'Auto' },
    });
});

test('rotator alerts on failures and logs to a retained group', () => {
    const app = new cdk.App();
    const stack = new InfraStack(app, 'MyTestStack', {
      code: lambda.Code.fromAsset(__dirname),
    });
    const template = Template.fromStack(stack);

    template.hasResourceProperties('AWS::SNS::Subscription', {
      Protocol: 'email',
      Endpoint: 'cmsd2@cantab.net',
    });
    template.hasResourceProperties('AWS::Events::Rule', {
      EventPattern: { source: ['aws.secretsmanager'], detail: { eventName: ['RotationFailed'] } },
    });
    template.hasResourceProperties('AWS::CloudWatch::Alarm', {
      MetricName: 'Errors',
      Threshold: 1,
    });
    template.hasResource('AWS::Logs::LogGroup', {
      Properties: { LogGroupName: '/aws/lambda/rotator', RetentionInDays: 365 },
      DeletionPolicy: 'Retain',
    });
    template.hasResourceProperties('AWS::Lambda::Function', {
      LoggingConfig: { LogGroup: { Ref: Match.anyValue() } },
    });
});

test('CI roles: master can deploy, branches and pull requests can only read', () => {
    const app = new cdk.App();
    const stack = new GithubActionsRoleStack(app, 'MyRoleStack', {
      env: { account: '123456789012', region: 'eu-west-1' },
      repository: 'cmsd2/rotator',
      branch: 'master',
    });
    const template = Template.fromStack(stack);
    const trust = (subjects: string[]) => ({
      Statement: [{
        Action: 'sts:AssumeRoleWithWebIdentity',
        Condition: {
          StringEquals: { 'token.actions.githubusercontent.com:aud': 'sts.amazonaws.com' },
          StringLike: { 'token.actions.githubusercontent.com:sub': subjects },
        },
        Effect: 'Allow',
        Principal: {
          Federated: 'arn:aws:iam::123456789012:oidc-provider/token.actions.githubusercontent.com',
        },
      }],
    });
    const bootstrap = (name: string) => `arn:aws:iam::123456789012:role/cdk-hnb659fds-${name}-role-123456789012-eu-west-1`;

    template.resourceCountIs('AWS::IAM::Role', 2);
    template.hasResourceProperties('AWS::IAM::Role', {
      RoleName: 'github-rotator',
      AssumeRolePolicyDocument: trust(['repo:cmsd2/rotator:ref:refs/heads/master']),
    });
    template.hasResourceProperties('AWS::IAM::Role', {
      RoleName: 'github-rotator-read',
      AssumeRolePolicyDocument: trust(['repo:cmsd2/rotator:ref:refs/heads/*', 'repo:cmsd2/rotator:pull_request']),
    });
    template.hasResourceProperties('AWS::IAM::Policy', {
      PolicyDocument: {
        Statement: [{
          Action: 'sts:AssumeRole',
          Effect: 'Allow',
          Resource: ['deploy', 'file-publishing', 'image-publishing', 'lookup'].map(bootstrap),
        }],
      },
    });
    template.hasResourceProperties('AWS::IAM::Policy', {
      PolicyDocument: {
        Statement: [{ Action: 'sts:AssumeRole', Effect: 'Allow', Resource: bootstrap('lookup') }],
      },
    });
});
