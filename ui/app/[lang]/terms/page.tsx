import type { Metadata } from "next";

type Section = {
    id: string;
    titleDe: string;
    titleEn: string;
    pointsDe: string[];
    pointsEn: string[];
};

const sections: Section[] = [
    {
        id: "service",
        titleDe: "Dienstbeschreibung",
        titleEn: "Service description",
        pointsDe: [
            "dokustatus ist ein freiwilliger und unentgeltlicher Online-Dienst, der den Status von Pass-, Personalausweis- oder eID-Anträgen der Landeshauptstadt München anhand der von Ihnen eingegebenen Vorgangsnummer regelmäßig abruft und Sie per E-Mail über Aktualisierungen informiert.",
            "Die Statusdaten stammen aus den Angeboten der Landeshauptstadt München (z. B. muenchen.de/pass). dokustatus ist kein Angebot einer Behörde und ersetzt keine verbindliche oder amtliche Auskunft."
        ],
        pointsEn: [
            "dokustatus is a voluntary and free online service that regularly checks the status of passport, national ID, or eID applications of the City of Munich using the process number you provide and notifies you by email about updates.",
            "All status data is retrieved from services of the City of Munich (e.g. muenchen.de/pass). dokustatus is not an authority and does not replace binding or official information."
        ]
    },
    {
        id: "use",
        titleDe: "Berechtigte Nutzung",
        titleEn: "Authorized use",
        pointsDe: [
            "Das Angebot richtet sich an Nutzerinnen und Nutzer in Deutschland.",
            "Sie dürfen nur Vorgangsnummern eingeben, zu deren Verwendung Sie berechtigt sind (z. B. Ihr eigener Antrag oder ein Antrag eines von Ihnen vertretenen Kindes).",
            "Die Eingabe fremder Vorgangsnummern ohne entsprechende Berechtigung ist untersagt."
        ],
        pointsEn: [
            "The service is intended for users in Germany.",
            "You may only submit process numbers you are entitled to use (e.g. your own application or that of a child you represent).",
            "Submitting process numbers without proper authorization is prohibited."
        ]
    },
    {
        id: "auth",
        titleDe: "Anmeldung und Benachrichtigungen",
        titleEn: "Sign-in and notifications",
        pointsDe: [
            "Die Anmeldung erfolgt über einen E-Mail-Link, der an die von Ihnen angegebene E-Mail-Adresse gesendet wird.",
            "Sie sind dafür verantwortlich, Ihr Postfach erreichbar und angemessen zu sichern.",
            "Benachrichtigungen zu Statusänderungen erfolgen ausschließlich per E-Mail."
        ],
        pointsEn: [
            "Sign-in is completed via an email link sent to the email address you provide.",
            "You are responsible for keeping your inbox accessible and reasonably secure.",
            "Notifications about status changes are sent exclusively by email."
        ]
    },
    {
        id: "data",
        titleDe: "Datenverarbeitung",
        titleEn: "Data processing",
        pointsDe: [
            "Zur Erbringung des Dienstes speichern wir Ihre E-Mail-Adresse, die von Ihnen eingegebene Vorgangsnummer sowie die dazugehörigen Statusinformationen, um regelmäßige Abrufe durchzuführen und Sie zu informieren.",
            "Die Daten werden ausschließlich zur Bereitstellung des Dienstes und zur Kommunikation mit Ihnen verwendet, soweit in der Datenschutzerklärung nicht anders beschrieben.",
            "Einzelheiten zur Verarbeitung personenbezogener Daten entnehmen Sie bitte der auf dokustatus.de veröffentlichten Datenschutzerklärung."
        ],
        pointsEn: [
            "To provide the service, we store your email address, the process number you submit, and the related status information in order to perform regular checks and notify you.",
            "The data is used solely to provide the service and to communicate with you, unless otherwise described in the privacy notice.",
            "For details on the processing of personal data, please refer to the privacy notice published on dokustatus.de."
        ]
    },
    {
        id: "warranty",
        titleDe: "Gewährleistung und Verfügbarkeit",
        titleEn: "Warranty and availability",
        pointsDe: [
            "Es wird keine Gewähr für die Richtigkeit, Vollständigkeit, Aktualität oder ständige Verfügbarkeit der über dokustatus bereitgestellten Informationen übernommen.",
            "Änderungen, Wartungen oder Störungen auf Seiten der Landeshauptstadt München oder ihrer Systeme (z. B. muenchen.de/pass) können dazu führen, dass Statusinformationen vorübergehend nicht oder nur eingeschränkt abrufbar sind.",
            "dokustatus ist ein zusätzliches Informationsangebot und ersetzt keine persönliche Vorsprache bei der Behörde, keine verbindliche Auskunft und keine rechtliche Beratung."
        ],
        pointsEn: [
            "No guarantee is given for the accuracy, completeness, timeliness, or continuous availability of information provided via dokustatus.",
            "Changes, maintenance, or outages on the side of the City of Munich or its systems (e.g. muenchen.de/pass) may result in status information being temporarily unavailable or limited.",
            "dokustatus is an additional information service and does not replace in-person visits to the authority, binding information, or legal advice."
        ]
    },
    {
        id: "liability",
        titleDe: "Haftung",
        titleEn: "Liability",
        pointsDe: [
            "Wir haften unbeschränkt für Schäden aus der Verletzung des Lebens, des Körpers oder der Gesundheit sowie für Schäden, die auf Vorsatz oder grober Fahrlässigkeit beruhen.",
            "Bei leicht fahrlässiger Verletzung wesentlicher Vertragspflichten (Kardinalpflichten) ist die Haftung auf den vertragstypischen, vorhersehbaren Schaden begrenzt.",
            "Im Übrigen ist die Haftung für leichte Fahrlässigkeit ausgeschlossen.",
            "Die zwingende gesetzliche Haftung, insbesondere nach dem Produkthaftungsgesetz, bleibt unberührt.",
            "Sie bleiben selbst verantwortlich für eigene Fristen und Dispositionen, insbesondere für Reiseplanung und die Gültigkeit von Ausweisdokumenten."
        ],
        pointsEn: [
            "We are fully liable for damages resulting from injury to life, body, or health, and for damages caused by intent or gross negligence.",
            "For minor negligent breaches of essential contractual obligations (cardinal duties), liability is limited to typical and foreseeable damages.",
            "Otherwise, liability for minor negligence is excluded.",
            "Mandatory statutory liability, including under product liability law, remains unaffected.",
            "You remain responsible for your own deadlines and decisions, in particular travel planning and the validity of identity documents."
        ]
    },
    {
        id: "changes",
        titleDe: "Änderungen und Beendigung des Dienstes",
        titleEn: "Changes and termination of the service",
        pointsDe: [
            "Sie können die Nutzung von dokustatus jederzeit einstellen; in diesem Fall werden für Ihre Vorgangsnummer keine weiteren Abrufe und Benachrichtigungen durchgeführt.",
            "Wir können den Dienst oder einzelne Funktionen jederzeit mit Wirkung für die Zukunft anpassen oder einstellen.",
            "Über wesentliche Änderungen der Nutzungsbedingungen oder des Dienstes informieren wir in geeigneter Form."
        ],
        pointsEn: [
            "You may stop using dokustatus at any time; in that case, no further checks or notifications will be carried out for your process number.",
            "We may modify or discontinue the service or individual features at any time with effect for the future.",
            "We will inform you appropriately about material changes to the terms of use or the service."
        ]
    },
    {
        id: "law",
        titleDe: "Anwendbares Recht",
        titleEn: "Governing law",
        pointsDe: [
            "Für die Nutzung von dokustatus gilt deutsches Recht.",
            "Zwingende verbraucherschützende Vorschriften des Staates, in dem Sie Ihren gewöhnlichen Aufenthalt haben, bleiben unberührt."
        ],
        pointsEn: [
            "German law applies to the use of dokustatus.",
            "Mandatory consumer protection provisions of the country in which you have your habitual residence remain unaffected."
        ]
    },
    {
        id: "contact",
        titleDe: "Kontakt",
        titleEn: "Contact",
        pointsDe: [
            "Die Kontaktmöglichkeiten entnehmen Sie bitte dem Impressum auf dokustatus.de."
        ],
        pointsEn: [
            "Please refer to the imprint on dokustatus.de for contact details."
        ]
    }
];

export const metadata: Metadata = {
    title: "Terms / Nutzungsbedingungen | dokustatus",
    description:
        "Bilingual terms and conditions for using the dokustatus document tracking service in Germany."
};

export default async function TermsPage({
                                            params
                                        }: {
    params: Promise<{ lang: "en" | "de" }>;
}) {
    // Consume params to keep parity with other routes; page content is bilingual by design.
    await params;

    return (
        <main>
            <h1>Allgemeine Nutzungsbedingungen / Terms of Service</h1>
            <p className="text-muted">
                Deutsch und Englisch stehen nebeneinander zu Ihrer Orientierung. Für die
                Nutzung in Deutschland ist die deutschsprachige Fassung maßgeblich. / German
                and English are shown side by side for convenience. For use in Germany, the
                German text prevails.
            </p>
            <div style={{ overflowX: "auto" }}>
                <table>
                    <thead>
                    <tr>
                        <th style={{ width: "50%" }}>Deutsch</th>
                        <th style={{ width: "50%" }}>English</th>
                    </tr>
                    </thead>
                    <tbody>
                    {sections.map((section) => (
                        <tr key={section.id}>
                            <td>
                                <h3 style={{ marginBottom: "8px" }}>{section.titleDe}</h3>
                                <ul style={{ paddingLeft: "18px", marginBottom: 0 }}>
                                    {section.pointsDe.map((point, idx) => (
                                        <li key={idx} style={{ marginBottom: "8px" }}>
                                            {point}
                                        </li>
                                    ))}
                                </ul>
                            </td>
                            <td>
                                <h3 style={{ marginBottom: "8px" }}>{section.titleEn}</h3>
                                <ul style={{ paddingLeft: "18px", marginBottom: 0 }}>
                                    {section.pointsEn.map((point, idx) => (
                                        <li key={idx} style={{ marginBottom: "8px" }}>
                                            {point}
                                        </li>
                                    ))}
                                </ul>
                            </td>
                        </tr>
                    ))}
                    </tbody>
                </table>
            </div>
            <p className="text-muted" style={{ marginTop: "var(--spacing-md)" }}>
                Stand / Last updated: November 2025
            </p>
        </main>
    );
}
