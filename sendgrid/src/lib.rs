use std::collections::HashMap;
use std::env;
use std::rc::Rc;

use data_encoding::BASE64;
use reqwest::blocking::{Client, Request};
use reqwest::{header, Method, StatusCode, Url};
use serde::Serialize;

/// Endpoint for the Sendgrid API.
const ENDPOINT: &str = "https://api.sendgrid.com/v3/";

/// Entrypoint for interacting with the SendGrid API.
pub struct SendGrid {
    key: String,
    domain: String,

    client: Rc<Client>,
}

impl SendGrid {
    /// Create a new SendGrid client struct. It takes a type that can convert into
    /// an &str (`String` or `Vec<u8>` for example). As long as the function is
    /// given a valid API Key and Secret your requests will work.
    pub fn new<K, D>(key: K, domain: D) -> Self
    where
        K: ToString,
        D: ToString,
    {
        let client = Client::builder().build();
        match client {
            Ok(c) => Self {
                key: key.to_string(),
                domain: domain.to_string(),

                client: Rc::new(c),
            },
            Err(e) => panic!("creating client failed: {:?}", e),
        }
    }

    /// Create a new SendGrid client struct from environment variables. It
    /// takes a type that can convert into
    /// an &str (`String` or `Vec<u8>` for example). As long as the function is
    /// given a valid API Key your requests will work.
    pub fn new_from_env() -> Self {
        let key = env::var("SENDGRID_API_KEY").unwrap();
        let domain = env::var("EMAIL_TEMPLATE_DOMAIN").unwrap();

        return SendGrid::new(key, domain);
    }

    /// Get the currently set API key.
    pub fn get_key(&self) -> &str {
        &self.key
    }

    fn request<B>(
        &self,
        method: Method,
        path: String,
        body: B,
        query: Option<Vec<(&str, String)>>,
    ) -> Request
    where
        B: Serialize,
    {
        let base = Url::parse(ENDPOINT).unwrap();
        let url = base.join(&path).unwrap();

        let bt = format!("Bearer {}", self.key);
        let bearer = header::HeaderValue::from_str(&bt).unwrap();

        // Set the default headers.
        let mut headers = header::HeaderMap::new();
        headers.append(header::AUTHORIZATION, bearer);
        headers.append(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );

        let mut rb = self.client.request(method.clone(), url).headers(headers);

        match query {
            None => (),
            Some(val) => {
                rb = rb.query(&val);
            }
        }

        // Add the body, this is to ensure our GET and DELETE calls succeed.
        if method != Method::GET && method != Method::DELETE {
            rb = rb.json(&body);
        }

        // Build the request.
        let request = rb.build().unwrap();

        return request;
    }

    /// Send an email.
    pub fn send_mail(&self, message: Message) {
        // Build the request.
        let request =
            self.request(Method::POST, "mail/send".to_string(), message, None);

        let resp = self.client.execute(request).unwrap();
        match resp.status() {
            StatusCode::ACCEPTED => (),
            s => panic!("received response status: {:?}", s),
        };
    }

    pub fn send_new_user(
        &self,
        primary_email: String,
        recovery_email: String,
        message: String,
    ) {
        // Create the message.
        let admin_email = format!("admin@{}", self.domain);

        let message = Message::new()
            .set_from(
                Email::new().set_email(&admin_email).set_name(&admin_email),
            )
            .set_subject(&format!("Your New Email Account: {}", primary_email))
            .add_content(
                Content::new()
                    .set_content_type("text/plain")
                    .set_value(&message),
            )
            .add_personalization(
                Personalization::new()
                    .add_to(
                        Email::new()
                            .set_email(&recovery_email)
                            .set_name(&recovery_email),
                    )
                    .add_cc(
                        Email::new()
                            .set_email(&format!("jess@{}", self.domain)),
                    ),
            );

        // Send the message.
        self.send_mail(message);
    }

    pub fn send_received_application(&self, email: &str, name: &str) {
        let careers_email = format!("careers@{}", self.domain);

        // Create the message.
        let message = Message::new()
            .set_from(
                Email::new()
                    .set_email(&careers_email)
                    .set_name(&careers_email),
            )
            .set_subject("Oxide Computer Company Application Received!")
            .add_content(Content::new().set_content_type("text/plain").set_value(
                "Thank you for submitting your application materials! We really appreciate all
the time and thought everyone puts into their application. We will be in touch
within the next couple weeks with more information.

Sincerely,
  The Oxide Team",
            ))
            .add_personalization(
                Personalization::new()
                    .add_to(Email::new().set_email(email).set_name(name))
                    .add_cc(Email::new().set_email(&careers_email)),
            );

        // Send the message.
        self.send_mail(message);
    }

    pub fn send(&self, body: &str, subject: &str, from: &str, to: &str) {
        // Create the message.
        let message = Message::new()
            .set_from(Email::new().set_email(&from).set_name(&from))
            .set_subject(&subject)
            .add_content(
                Content::new()
                    .set_content_type("text/plain")
                    .set_value(&body),
            )
            .add_personalization(
                Personalization::new()
                    .add_to(Email::new().set_email(&to).set_name(&to)),
            );

        // Send the message.
        self.send_mail(message);
    }

    pub fn send_uploaded_zoom_dump(&self, drive_url: &str) {
        let drive_email = format!("drive@{}", self.domain);

        // Create the message.
        let message = Message::new()
            .set_from(Email::new().set_email(&drive_email).set_name(&drive_email))
            .set_subject("New Zoom meeting video upload!")
            .add_content(
                Content::new()
                    .set_content_type("text/plain")
                    .set_value(&format!(
                        "Zoom videos have been uploaded to: {}. You might want to sort them!",
                        drive_url
                    )),
            )
            .add_personalization(
                Personalization::new()
                    .add_to(
                        Email::new()
                            .set_email(&format!("jess@{}", self.domain))
                            .set_name(&format!("jess@{}", self.domain)),
                    )
                    .add_cc(Email::new().set_email(&drive_email)),
            );

        // Send the message.
        self.send_mail(message);
    }

    pub fn send_new_applicant_notification(
        &self,
        name: String,
        message: String,
    ) {
        let applications_email = format!("applications@{}", self.domain);
        let all_email = format!("all@{}", self.domain);

        // Create the message.
        let message = Message::new()
            .set_from(
                Email::new()
                    .set_email(&applications_email)
                    .set_name(&applications_email),
            )
            .set_subject(&format!("New Application: {}", name))
            .add_content(
                Content::new()
                    .set_content_type("text/plain")
                    .set_value(&message),
            )
            .add_personalization(Personalization::new().add_to(
                Email::new().set_email(&all_email).set_name(&all_email),
            ));

        // Send the message.
        self.send_mail(message);
    }
}

/// The main structure for a V3 API mail send call. This is composed of many other smaller
/// structures used to add lots of customization to your message.
#[derive(Default, Serialize)]
pub struct Message {
    from: Email,
    subject: String,
    personalizations: Vec<Personalization>,

    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<Vec<Content>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    attachments: Option<Vec<Attachment>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    template_id: Option<String>,
}

/// An email with a required address and an optional name field.
#[derive(Clone, Default, Serialize)]
pub struct Email {
    email: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// The body of an email with the content type and the message.
#[derive(Clone, Default, Serialize)]
pub struct Content {
    #[serde(rename = "type")]
    content_type: String,
    value: String,
}

/// A personalization block for a V3 message. It has to at least contain one email as a to
/// address. All other fields are optional.
#[derive(Default, Serialize)]
pub struct Personalization {
    to: Vec<Email>,

    #[serde(skip_serializing_if = "Option::is_none")]
    cc: Option<Vec<Email>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    bcc: Option<Vec<Email>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    subject: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    headers: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    substitutions: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    custom_args: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    dynamic_template_data: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    send_at: Option<u64>,
}

/// An attachment block for a V3 message. Content and filename are required. If the
/// mime_type is unspecified, the email will use Sendgrid's default for attachments
/// which is 'application/octet-stream'.
#[derive(Default, Serialize)]
pub struct Attachment {
    content: String,

    filename: String,

    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    mime_type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    disposition: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    content_id: Option<String>,
}

impl Message {
    /// Construct a new V3 message.
    pub fn new() -> Message {
        Message::default()
    }

    /// Set the from address.
    pub fn set_from(mut self, from: Email) -> Message {
        self.from = from;
        self
    }

    /// Set the subject.
    pub fn set_subject(mut self, subject: &str) -> Message {
        self.subject = String::from(subject);
        self
    }

    /// Set the template id.
    pub fn set_template_id(mut self, template_id: &str) -> Message {
        self.template_id = Some(String::from(template_id));
        self
    }

    /// Add content to the message.
    pub fn add_content(mut self, c: Content) -> Message {
        match self.content {
            None => {
                let mut content = Vec::new();
                content.push(c);
                self.content = Some(content);
            }
            Some(ref mut content) => content.push(c),
        };
        self
    }

    /// Add a personalization to the message.
    pub fn add_personalization(mut self, p: Personalization) -> Message {
        self.personalizations.push(p);
        self
    }

    /// Add an attachment to the message.
    pub fn add_attachment(mut self, a: Attachment) -> Message {
        match self.attachments {
            None => {
                let mut attachments = Vec::new();
                attachments.push(a);
                self.attachments = Some(attachments);
            }
            Some(ref mut attachments) => attachments.push(a),
        };
        self
    }
}

impl Email {
    /// Construct a new email type.
    pub fn new() -> Email {
        Email::default()
    }

    /// Set the address for this email.
    pub fn set_email(mut self, email: &str) -> Email {
        self.email = String::from(email);
        self
    }

    /// Set an optional name.
    pub fn set_name(mut self, name: &str) -> Email {
        self.name = Some(String::from(name));
        self
    }
}

impl Content {
    /// Construct a new content type.
    pub fn new() -> Content {
        Content::default()
    }

    /// Set the type of this content.
    pub fn set_content_type(mut self, content_type: &str) -> Content {
        self.content_type = String::from(content_type);
        self
    }

    /// Set the corresponding message for this content.
    pub fn set_value(mut self, value: &str) -> Content {
        self.value = String::from(value);
        self
    }
}

impl Personalization {
    /// Construct a new personalization block for this message.
    pub fn new() -> Personalization {
        Personalization::default()
    }

    /// Add a to field.
    pub fn add_to(mut self, to: Email) -> Personalization {
        self.to.push(to);
        self
    }

    /// Add a CC field.
    pub fn add_cc(mut self, cc: Email) -> Personalization {
        match self.cc {
            None => {
                let mut ccs = Vec::new();
                ccs.push(cc);
                self.cc = Some(ccs);
            }
            Some(ref mut c) => {
                c.push(cc);
            }
        }
        self
    }

    /// Add a BCC field.
    pub fn add_bcc(mut self, bcc: Email) -> Personalization {
        match self.bcc {
            None => {
                let mut bccs = Vec::new();
                bccs.push(bcc);
                self.bcc = Some(bccs);
            }
            Some(ref mut b) => {
                b.push(bcc);
            }
        }
        self
    }

    /// Add a headers field.
    pub fn add_headers(
        mut self,
        headers: HashMap<String, String>,
    ) -> Personalization {
        match self.headers {
            None => {
                let mut h = HashMap::new();
                for (name, value) in headers {
                    h.insert(name, value);
                }
                self.headers = Some(h);
            }
            Some(ref mut h) => {
                h.extend(headers);
            }
        }
        self
    }

    /// Add a dynamic template data field.
    pub fn add_dynamic_template_data(
        mut self,
        dynamic_template_data: HashMap<String, String>,
    ) -> Personalization {
        match self.dynamic_template_data {
            None => {
                let mut h = HashMap::new();
                for (name, value) in dynamic_template_data {
                    h.insert(name, value);
                }
                self.dynamic_template_data = Some(h);
            }
            Some(ref mut h) => {
                h.extend(dynamic_template_data);
            }
        }
        self
    }
}

impl Attachment {
    /// Construct a new attachment for this message.
    pub fn new() -> Attachment {
        Attachment::default()
    }

    /// The raw body of the attachment.
    pub fn set_content(mut self, c: &[u8]) -> Attachment {
        self.content = BASE64.encode(c);
        self
    }

    /// The base64 body of the attachment.
    pub fn set_base64_content(mut self, c: &str) -> Attachment {
        self.content = String::from(c);
        self
    }

    /// Sets the filename for the attachment.
    pub fn set_filename(mut self, filename: &str) -> Attachment {
        self.filename = filename.into();
        self
    }

    /// Set an optional mime type. Sendgrid will default to 'application/octet-stream'
    /// if unspecified.
    pub fn set_mime_type(mut self, mime: &str) -> Attachment {
        self.mime_type = Some(String::from(mime));
        self
    }
}
