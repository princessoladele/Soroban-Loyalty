import { Metadata } from "next";
import { api } from "@/lib/api";

export async function generateMetadata({ params }: { params: { id: string } }): Promise<Metadata> {
  const id = parseInt(params.id);
  const baseUrl = process.env.NEXT_PUBLIC_API_URL || "http://localhost:3000";
  try {
    const { campaign } = await api.getCampaign(id);
    const merchantShort = `${campaign.merchant.slice(0, 8)}...${campaign.merchant.slice(-4)}`;
    const title = campaign.name ? `${campaign.name} - SorobanLoyalty` : `Campaign #${id} - SorobanLoyalty`;
    const description = campaign.name
      ? `Join "${campaign.name}" campaign and earn ${campaign.reward_amount} LYT rewards from merchant ${merchantShort}. On-chain loyalty platform on Stellar.`
      : `Earn ${campaign.reward_amount} LYT rewards from merchant ${merchantShort}. Join now!`;

    return {
      title,
      description,
      openGraph: {
        title,
        description,
        type: "website",
        url: `${baseUrl}/campaigns/${id}`,
        siteName: "SorobanLoyalty",
        images: campaign.image_url ? [
          {
            url: campaign.image_url,
            width: 1200,
            height: 630,
            alt: campaign.name || `Campaign #${id}`,
          }
        ] : [],
      },
      twitter: {
        card: "summary_large_image",
        title,
        description,
        images: campaign.image_url ? [campaign.image_url] : [],
      },
      other: {
        "og:type": "website",
        "og:site_name": "SorobanLoyalty",
      }
    };
  } catch (e) {
    return {
      title: "Campaign Not Found - SorobanLoyalty",
      description: "The campaign you are looking for does not exist or has been removed.",
    };
  }
}

export default function CampaignPage({ params }: { params: { id: string } }) {
  return <CampaignDetailClient id={parseInt(params.id)} />;
}
