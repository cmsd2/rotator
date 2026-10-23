import * as cdk from 'aws-cdk-lib';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import { Match, Template } from 'aws-cdk-lib/assertions';
import { InfraStack } from '../lib/infra-stack';

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
