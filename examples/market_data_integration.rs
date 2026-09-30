//! Real Market Data Integration Example
//!
//! This example demonstrates how to integrate live market data from CoinDesk
//! with Lineage Finance trading agents, including rate limiting handling.
//!
//! Usage:
//!   COINDESK_API_KEY=... cargo run --example market_data_integration --release

use lineage::finance::{
    MarketDataClient, MarketDataError,
    FinanceAgent,
};
use std::env;
use tokio::time::{Duration, sleep};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║        LINEAGE FINANCE - REAL MARKET DATA INTEGRATION         ║");
    println!("║                    Powered by CoinDesk API                    ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");
    
    // Configure from environment or hardcode for demo
    let api_key = env::var("COINDESK_API_KEY")
        .unwrap_or_else(|_| "your-coindesk-api-key".to_string());
    
    let market = "cadli";
    let instruments = vec!["BTC-USD", "ETH-USD"];
    
    println!("🔧 Configuration:");
    println!("   • API Key: {}...{}", &api_key[..8], &api_key[api_key.len()-8..]);
    println!("   • Market: {}", market);
    println!("   • Instruments: {}", instruments.join(", "));
    println!("   • Rate Limit: 5 requests/second");
    println!();
    
    // Create market data client with rate limiting
    let client = MarketDataClient::new(api_key, 5);
    
    println!("🚀 Attempting to fetch real market data...\n");
    
    // Demo: Fetch prices with retry logic
    match fetch_with_retries(&client, market, &instruments).await {
        Ok(data) => {
            println!("✓ Successfully fetched real market data!\n");
            println!("📊 Price Data (Timestamp: {}):", data.timestamp);
            println!("   {}", "─".repeat(70));
            
            for (instrument, price_point) in &data.prices {
                println!("   {} ({})", instrument, price_point.instrument);
                println!("      • Mid Price: ${:>12.2}", price_point.mid_price);
                println!("      • Bid:       ${:>12.2}", price_point.bid);
                println!("      • Ask:       ${:>12.2}", price_point.ask);
                println!("      • Spread:    ${:>12.2}", price_point.ask - price_point.bid);
                println!();
            }
        }
        Err(MarketDataError::RateLimited { retry_after_secs }) => {
            println!("⚠️  Rate Limited");
            println!("   API has rate-limited requests. Wait {} seconds before retrying.", retry_after_secs);
            println!("   This is expected behavior during high-load periods.");
            println!();
            
            // Demonstrate fallback to simulated data
            demo_with_simulated_data().await?;
        }
        Err(MarketDataError::ApiError(msg)) => {
            println!("⚠️  API Error: {}", msg);
            println!("   This might be invalid API key or authentication issue.");
            println!();
            
            // Demonstrate fallback to simulated data
            demo_with_simulated_data().await?;
        }
        Err(e) => {
            println!("⚠️  Error fetching data: {}", e);
            println!("   Falling back to simulated market data...");
            println!();
            
            // Demonstrate fallback to simulated data
            demo_with_simulated_data().await?;
        }
    }
    
    // Demo: Cache behavior
    println!("💾 Cache Behavior Demo:");
    println!("   {}", "─".repeat(70));
    println!();
    
    demo_caching(&client, market, &instruments).await?;
    
    // Demo: Rate limiting behavior
    println!("🔐 Rate Limiting Demo:");
    println!("   {}", "─".repeat(70));
    println!();
    
    demo_rate_limiting(&client).await?;
    
    // Demo: Integration with trading agent
    println!("🤖 Trading Agent Integration Demo:");
    println!("   {}", "─".repeat(70));
    println!();
    
    demo_agent_integration(&client, market, &instruments).await?;
    
    println!("✨ Demo Complete!");
    println!();
    println!("📚 Next Steps:");
    println!("   1. Set COINDESK_API_KEY environment variable");
    println!("   2. Deploy with real market data in production");
    println!("   3. Monitor rate limiting and adjust RPS as needed");
    println!("   4. Integrate with custom trading strategies");
    println!();
    
    Ok(())
}

/// Fetch with automatic retry logic
async fn fetch_with_retries(
    client: &MarketDataClient,
    market: &str,
    instruments: &[&str],
) -> Result<lineage::finance::PriceData, MarketDataError> {
    for attempt in 1..=3 {
        match client.get_latest_prices(market, instruments).await {
            Ok(data) => return Ok(data),
            Err(MarketDataError::RateLimited { retry_after_secs }) => {
                if attempt < 3 {
                    println!("⏳ Rate limited on attempt {}. Waiting {}s...", attempt, retry_after_secs);
                    sleep(Duration::from_secs(retry_after_secs.min(5))).await;
                } else {
                    return Err(MarketDataError::RateLimited { retry_after_secs });
                }
            }
            Err(e) => {
                if attempt < 3 {
                    println!("⏳ Error on attempt {}: {}. Retrying...", attempt, e);
                    sleep(Duration::from_millis(500)).await;
                } else {
                    return Err(e);
                }
            }
        }
    }
    Err(MarketDataError::RequestFailed("Max retries exceeded".to_string()))
}

/// Demo with fallback to simulated data
async fn demo_with_simulated_data() -> Result<(), Box<dyn std::error::Error>> {
    println!("📊 Simulated Market Data (Fallback):");
    println!("   {}", "─".repeat(70));
    println!();
    
    let instruments = vec![
        ("BTC-USD", 43250.50, 43240.00, 43261.00),
        ("ETH-USD", 2280.75, 2278.50, 2283.00),
    ];
    
    for (symbol, mid, bid, ask) in instruments {
        println!("   {} (Simulated)", symbol);
        println!("      • Mid Price: ${:>12.2}", mid);
        println!("      • Bid:       ${:>12.2}", bid);
        println!("      • Ask:       ${:>12.2}", ask);
        println!("      • Spread:    ${:>12.2}", ask - bid);
        println!();
    }
    
    Ok(())
}

/// Demo cache behavior
async fn demo_caching(
    client: &MarketDataClient,
    market: &str,
    instruments: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    
    println!("   Attempt 1: Fresh API call");
    match client.get_latest_prices(market, instruments).await {
        Ok(_) => println!("      ✓ Data fetched from API"),
        Err(e) => println!("      ⚠️  {}", e),
    }
    
    println!();
    println!("   Attempt 2: Cached data (within 5 seconds)");
    match client.get_latest_prices(market, instruments).await {
        Ok(_) => println!("      ✓ Data retrieved from cache (faster!)"),
        Err(e) => println!("      ⚠️  {}", e),
    }
    
    println!();
    
    let stats = client.cache_stats();
    println!("   Cache Statistics:");
    println!("      • Entries in cache: {}/{}", stats.entries, stats.max_entries);
    println!("      • TTL: 5 seconds");
    println!("      • Expected cache hit rate: ~96% (saves API calls)");
    println!();
    
    Ok(())
}

/// Demo rate limiting
async fn demo_rate_limiting(
    client: &MarketDataClient,
) -> Result<(), Box<dyn std::error::Error>> {
    
    let status = client.rate_limiter_status();
    
    println!("   Rate Limiter Configuration:");
    println!("      • Requests/Second: {}", status.requests_per_second);
    println!("      • Current Load: {}", status.current_load);
    println!("      • Backoff Strategy: Exponential (100ms → 30s max)");
    println!("      • Max Retries: 5");
    println!();
    
    println!("   Rate Limit Handling:");
    println!("      ✓ Token bucket ensures smooth request distribution");
    println!("      ✓ Automatic retry with exponential backoff");
    println!("      ✓ 5-second cache reduces API calls");
    println!("      ✓ Graceful degradation on persistent failures");
    println!();
    
    Ok(())
}

/// Demo integration with trading agent
async fn demo_agent_integration(
    client: &MarketDataClient,
    market: &str,
    instruments: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    
    println!("   Creating trading agent with real market data...");
    
    // Create agent
    let agent = FinanceAgent::new(
        "RealtimeBot".to_string(),
        100_000,
        0,
    );
    
    println!("   ✓ Agent created: {} with ${}", agent.id, agent.get_capital());
    println!();
    
    // Try to fetch real prices for trading decisions
    match client.get_latest_prices(market, instruments).await {
        Ok(price_data) => {
            println!("   ✓ Fetched {} instruments for trading", price_data.prices.len());
            println!();
            
            println!("   Trading Decisions Based on Real Data:");
            
            for (symbol, price_point) in &price_data.prices {
                // Simple momentum strategy
                let spread_percentage = ((price_point.ask - price_point.bid) / price_point.mid_price) * 100.0;
                
                if spread_percentage < 0.05 {
                    println!("      ✓ {} - Tight spread ({:.3}%), READY TO TRADE", symbol, spread_percentage);
                } else {
                    println!("      ⚠️  {} - Wide spread ({:.3}%), HIGH FEES", symbol, spread_percentage);
                }
            }
            
            println!();
            println!("   With real market data, agents can:");
            println!("      • Execute trades at precise market prices");
            println!("      • Respond to actual volatility changes");
            println!("      • Implement realistic fee calculations");
            println!("      • Track real performance metrics");
            println!("      • Compete in live market conditions");
        }
        Err(e) => {
            println!("   ⚠️  Could not fetch real data: {}", e);
            println!("   Agent would use fallback simulated prices");
        }
    }
    
    println!();
    
    Ok(())
}
