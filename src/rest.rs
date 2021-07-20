use serde

#[derive(Serialize, Deserialize)]
struct OfferRequest {
  offer_sdp: String
}

#[derive(Serialize, Deserialize)]
struct OfferResponse {
  answer_sdp: String
}