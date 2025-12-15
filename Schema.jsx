import React from "react";

const defaultOrigin =
  typeof window !== "undefined"
    ? window.location.origin
    : "https://highgroundcannabiscafe.com";

const defaultBusiness = {
  "@context": "https://schema.org",
  "@type": "CannabisStore",
  name: "Highground Cannabis Cafe | รังสิต ม.กรุงเทพ | Weed Store",
  url: defaultOrigin,
  image: [
    "https://maps.google.com/maps/vt?pb=!1m5!1s5683579636760481711:0x4b0c6fb1f8b0c17f!5m2!1s2024-06-01!2i2!3m1!1e3",
    "https://maps.google.com/maps/api/staticmap?center=14.0604,100.6142&zoom=16&size=640x400&markers=color:green%7C14.0604,100.6142",
  ],
  contactPoint: [
    {
      "@type": "ContactPoint",
      telephone: "+66 61 869 6057",
      contactType: "customer service",
    },
    {
      "@type": "ContactPoint",
      telephone: "+66 82 286 5101",
      contactType: "sales",
    },
  ],
  priceRange: "฿฿",
  address: {
    "@type": "PostalAddress",
    streetAddress: "92/12 Phahonyothin Rd., Moo 4",
    addressLocality: "Khlong Nueng, Khlong Luang",
    addressRegion: "Pathum Thani",
    postalCode: "12120",
    addressCountry: "TH",
  },
  hasMap: "https://maps.google.com/?cid=5683579636760481711",
  identifier: [
    {
      "@type": "PropertyValue",
      name: "Google Place ID",
      value: "ChIJFX9pbu-BHTERr4_AFZEh4E4",
    },
    {
      "@type": "PropertyValue",
      name: "Google CID",
      value: "5683579636760481711",
    },
    {
      "@type": "PropertyValue",
      name: "Plus Code",
      value: "2JJ8+75",
    },
  ],
  openingHoursSpecification: [
    ...["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"].flatMap(
      (dayOfWeek) => [
        {
          "@type": "OpeningHoursSpecification",
          dayOfWeek,
          opens: "00:00",
          closes: "02:30",
        },
        {
          "@type": "OpeningHoursSpecification",
          dayOfWeek,
          opens: "08:00",
          closes: "23:59",
        },
      ],
    ),
  ],
  areaServed: [
    { "@type": "City", name: "Khlong Luang" },
    { "@type": "City", name: "Pathum Thani" },
    { "@type": "Place", name: "Bangkok University Rangsit Campus" },
  ],
  sameAs: [
    "https://www.google.com/maps?cid=5683579636760481711",
    "https://maps.google.com/?cid=5683579636760481711",
  ],
};

const defaultFaq = [
  {
    question: "ร้านเปิดบริการเวลาไหน? / What are your opening hours?",
    answer:
      "เราเปิดทุกวัน 08:00–23:59 และเที่ยงคืนถึง 02:30 ของวันถัดไป (08:00–23:59 and 00:00–02:30 daily).",
  },
  {
    question: "อยู่ตรงไหนและจอดรถได้ที่ใด? / Where are you located and can I park nearby?",
    answer:
      "Highground Cannabis Cafe ตั้งอยู่ที่ 92/12 ถนนพหลโยธิน คลองหนึ่ง คลองหลวง ปทุมธานี 12120 ใกล้มหาวิทยาลัยกรุงเทพ (รังสิต) มีที่จอดรถหน้าร้านและใกล้ๆ",
  },
  {
    question: "มีบริการเดลิเวอรีหรือสั่งออนไลน์ไหม? / Do you offer delivery or online ordering?",
    answer:
      "สามารถโทรสอบถามสินค้าหรือสั่งล่วงหน้าทางโทรศัพท์ที่ +66 61 869 6057 หรือ +66 82 286 5101 เพื่อเตรียมของไว้รับหน้าร้านได้",
  },
];

const defaultCollections = [
  {
    name: "Featured Flower & Pre-rolls",
    description: "Top-shelf flower and ready-to-enjoy pre-rolls picked for Rangsit customers.",
    url: `${defaultOrigin}/collections/flower`,
    items: [
      {
        name: "Rangsit Hybrid 3.5g",
        url: `${defaultOrigin}/products/rangsit-hybrid-3-5g`,
        image: `${defaultOrigin}/images/products/rangsit-hybrid-3-5g.jpg`,
      },
      {
        name: "Bangkok Kush Pre-roll 1g",
        url: `${defaultOrigin}/products/bangkok-kush-pre-roll-1g`,
        image: `${defaultOrigin}/images/products/bangkok-kush-pre-roll-1g.jpg`,
      },
      {
        name: "Cafe Sativa 7g",
        url: `${defaultOrigin}/products/cafe-sativa-7g`,
        image: `${defaultOrigin}/images/products/cafe-sativa-7g.jpg`,
      },
    ],
  },
  {
    name: "Vapes & Extracts",
    description: "Best-selling cartridges and live extracts popular with Bangkok University students and locals.",
    url: `${defaultOrigin}/collections/vapes`,
    items: [
      {
        name: "Live Resin 1g Cart",
        url: `${defaultOrigin}/products/live-resin-1g-cart`,
        image: `${defaultOrigin}/images/products/live-resin-1g-cart.jpg`,
      },
      {
        name: "CBD:THC 1:1 Disposable",
        url: `${defaultOrigin}/products/cbd-thc-1-1-disposable`,
        image: `${defaultOrigin}/images/products/cbd-thc-1-1-disposable.jpg`,
      },
    ],
  },
];

const createBreadcrumbList = (origin = defaultOrigin, pathname = "/") => {
  const trimmedPath = pathname.split("?")[0];
  const segments = trimmedPath.split("/").filter(Boolean);

  const itemListElement = [
    {
      "@type": "ListItem",
      position: 1,
      name: "Home",
      item: origin,
    },
    ...segments.map((segment, index) => {
      const position = index + 2;
      const url = `${origin}/${segments.slice(0, index + 1).join("/")}`;
      const name = segment.replace(/[-_]+/g, " ").replace(/\b\w/g, (char) => char.toUpperCase());

      return {
        "@type": "ListItem",
        position,
        name,
        item: url,
      };
    }),
  ];

  return {
    "@context": "https://schema.org",
    "@type": "BreadcrumbList",
    itemListElement,
  };
};

const createFaqSchema = (faq = defaultFaq) => ({
  "@context": "https://schema.org",
  "@type": "FAQPage",
  mainEntity: faq.map((entry) => ({
    "@type": "Question",
    name: entry.question,
    acceptedAnswer: {
      "@type": "Answer",
      text: entry.answer,
    },
  })),
});

const createCollectionSchema = (collections = defaultCollections) => ({
  "@context": "https://schema.org",
  "@type": "CollectionPage",
  name: "Curated Cannabis Collections",
  description:
    "Browse featured flower, vapes, and new arrivals curated by Highground Cannabis Cafe in Rangsit.",
  hasPart: collections.map((collection) => ({
    "@type": "ItemList",
    name: collection.name,
    description: collection.description,
    url: collection.url,
    itemListElement: collection.items.map((item, index) => ({
      "@type": "ListItem",
      position: index + 1,
      url: item.url,
      name: item.name,
      image: item.image,
    })),
  })),
});

const Schema = ({
  business = defaultBusiness,
  faq = defaultFaq,
  collections = defaultCollections,
  origin = defaultOrigin,
  pathname,
}) => {
  const effectivePathname = pathname ?? (typeof window !== "undefined" ? window.location.pathname : "/");

  const schemas = [
    { ...business, "@context": "https://schema.org" },
    createBreadcrumbList(origin, effectivePathname),
    createFaqSchema(faq),
    createCollectionSchema(collections),
  ];

  return (
    <>
      {schemas.map((schema, index) => (
        <script
          key={`${schema["@type"]}-${index}`}
          type="application/ld+json"
          dangerouslySetInnerHTML={{ __html: JSON.stringify(schema) }}
        />
      ))}
    </>
  );
};

export default Schema;
