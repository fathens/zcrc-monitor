use super::GrpcClient;
use super::proto::portfolio_service_client::PortfolioServiceClient;
use super::proto::GetEvaluationPeriodsRequest;
use chrono::{DateTime, Utc};

pub struct EvaluationPeriodItem {
    pub id: i32,
    pub period_id: String,
    pub start_time: DateTime<Utc>,
    pub initial_value: String,
    pub selected_tokens: Vec<String>,
}

fn timestamp_to_datetime(ts: prost_types::Timestamp) -> DateTime<Utc> {
    DateTime::from_timestamp(ts.seconds, ts.nanos as u32).unwrap_or_default()
}

fn proto_to_item(ep: super::proto::EvaluationPeriod) -> EvaluationPeriodItem {
    EvaluationPeriodItem {
        id: ep.id,
        period_id: ep.period_id,
        start_time: ep.start_time.map(timestamp_to_datetime).unwrap_or_default(),
        initial_value: ep.initial_value,
        selected_tokens: ep.selected_tokens,
    }
}

pub async fn get_evaluation_periods(
    client: &GrpcClient,
    page: i32,
    page_size: i32,
) -> Result<(Vec<EvaluationPeriodItem>, i64), String> {
    let channel = client.channel().await.map_err(|e| format!("{e:?}"))?;
    let mut svc = PortfolioServiceClient::new(channel);
    let response = svc
        .get_evaluation_periods(GetEvaluationPeriodsRequest { page, page_size })
        .await
        .map_err(|e| format!("{e:?}"))?;
    let inner = response.into_inner();
    let items = inner.periods.into_iter().map(proto_to_item).collect();
    Ok((items, inner.total_count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_to_datetime_normal() {
        let ts = prost_types::Timestamp {
            seconds: 1700000000,
            nanos: 0,
        };
        let dt = timestamp_to_datetime(ts);
        assert_eq!(dt.timestamp(), 1700000000);
    }

    #[test]
    fn test_timestamp_to_datetime_epoch() {
        let ts = prost_types::Timestamp {
            seconds: 0,
            nanos: 0,
        };
        let dt = timestamp_to_datetime(ts);
        assert_eq!(dt.timestamp(), 0);
    }

    #[test]
    fn test_timestamp_to_datetime_with_nanos() {
        let ts = prost_types::Timestamp {
            seconds: 1700000000,
            nanos: 500_000_000,
        };
        let dt = timestamp_to_datetime(ts);
        assert_eq!(dt.timestamp(), 1700000000);
        assert_eq!(dt.timestamp_subsec_nanos(), 500_000_000);
    }

    #[test]
    fn test_proto_to_item() {
        let proto = super::super::proto::EvaluationPeriod {
            id: 42,
            period_id: "eval_test".to_string(),
            start_time: Some(prost_types::Timestamp {
                seconds: 1700000000,
                nanos: 0,
            }),
            initial_value: "123456".to_string(),
            selected_tokens: vec!["a".to_string(), "b".to_string()],
        };
        let item = proto_to_item(proto);
        assert_eq!(item.id, 42);
        assert_eq!(item.period_id, "eval_test");
        assert_eq!(item.initial_value, "123456");
        assert_eq!(item.selected_tokens, vec!["a", "b"]);
        assert_eq!(item.start_time.timestamp(), 1700000000);
    }

    #[test]
    fn test_proto_to_item_no_start_time() {
        let proto = super::super::proto::EvaluationPeriod {
            id: 1,
            period_id: "eval_none".to_string(),
            start_time: None,
            initial_value: "0".to_string(),
            selected_tokens: vec![],
        };
        let item = proto_to_item(proto);
        assert_eq!(item.start_time, DateTime::<Utc>::default());
    }
}
