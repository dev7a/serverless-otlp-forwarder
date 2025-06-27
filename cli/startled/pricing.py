# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "boto3",
# ]
# ///
import boto3
import json

def get_lambda_compute_pricing(region_code='us-east-1', architecture='x86'):
    pricing_client = boto3.client('pricing', region_name='us-east-1')
    
    # Determine usage type based on architecture
    if architecture.lower() == 'arm':
        usage_type = 'Lambda-GB-Second-ARM'
    else:
        usage_type = 'Lambda-GB-Second'
    
    try:
        response = pricing_client.get_products(
            ServiceCode='AWSLambda',
            Filters=[
                {
                    'Type': 'TERM_MATCH',
                    'Field': 'usagetype',
                    'Value': usage_type
                },
                {
                    'Type': 'TERM_MATCH',
                    'Field': 'regionCode',
                    'Value': region_code
                }
            ]
        )
        
        for price_item in response['PriceList']:
            price_data = json.loads(price_item)
            print(f"Architecture: {architecture}")
            print(f"Region: {region_code}")
            print(f"Usage Type: {price_data['product']['attributes']['usagetype']}")
            
            # Extract tiered pricing
            terms = price_data['terms']['OnDemand']
            for term_key, term_value in terms.items():
                price_dimensions = term_value['priceDimensions']
                for dim_key, dim_value in price_dimensions.items():
                    print(f"Tier: {dim_value['description']}")
                    print(f"Range: {dim_value['beginRange']} - {dim_value['endRange']} GB-seconds")
                    print(f"Price per GB-second: ${dim_value['pricePerUnit']['USD']}")
                    print("---")
                    
    except Exception as e:
        print(f"Error: {e}")

# Examples
get_lambda_compute_pricing('us-east-1', 'x86')
get_lambda_compute_pricing('us-east-1', 'arm')
