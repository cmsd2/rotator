import * as cdk from 'aws-cdk-lib';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import { Template } from 'aws-cdk-lib/assertions';
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
