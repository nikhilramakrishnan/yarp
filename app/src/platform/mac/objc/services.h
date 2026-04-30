// Our class for handling NSServices messages.
@interface YarpServicesProvider : NSObject
@end

// Functions implemented in Rust.
id yarp_services_provider_custom_url_scheme();
void yarp_app_open_urls(id app, id urls);
